//! Runtime registry of active implant integrations.
//!
//! Load pipeline:
//!
//!   active plugin roots
//!      -> descriptor
//!      -> manifest
//!      -> worker process
//!      -> WorkerPluginIntegration
//!      -> index by integration id
//!      -> index by implant type
//!
//! Failures are accumulated into `load_errors` so one broken plugin does not
//! prevent the rest of the active set from loading.

use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
    sync::Arc,
};

use super::{
    loaded::WorkerPluginIntegration, package::PluginStore, types::ImplantIntegration,
    worker::WorkerPluginClient,
};

#[derive(Clone)]
pub struct ImplantIntegrationRegistry {
    by_id: Arc<HashMap<String, Arc<dyn ImplantIntegration>>>,
    by_implant_type: Arc<HashMap<String, Arc<dyn ImplantIntegration>>>,
    load_errors: Arc<Vec<String>>,
}

impl ImplantIntegrationRegistry {
    /// Loads every active plugin into a runtime integration registry.
    ///
    /// Duplicate integration ids or implant types are rejected at registry
    /// build time because dispatch later assumes a one-to-one mapping.
    pub fn load(store: &PluginStore) -> Self {
        let mut by_id = HashMap::new();
        let mut by_implant_type = HashMap::new();
        let mut load_errors = Vec::new();

        match store.active_plugin_roots() {
            Ok(package_roots) => {
                for package_root in package_roots {
                    let descriptor = match store.load_descriptor(&package_root) {
                        Ok(descriptor) => descriptor,
                        Err(err) => {
                            load_errors.push(format!(
                                "failed to read plugin package at '{}': {err}",
                                package_root.display()
                            ));
                            continue;
                        }
                    };
                    let manifest = match descriptor.load_manifest(&package_root) {
                        Ok(manifest) => manifest,
                        Err(err) => {
                            load_errors.push(format!(
                                "failed to load manifest for plugin '{}': {err}",
                                descriptor.plugin_id
                            ));
                            continue;
                        }
                    };
                    let worker = match WorkerPluginClient::start(&package_root, &descriptor) {
                        Ok(worker) => worker,
                        Err(err) => {
                            load_errors.push(format!(
                                "failed to start plugin '{}': {err}",
                                descriptor.plugin_id
                            ));
                            continue;
                        }
                    };
                    let integration: Arc<dyn ImplantIntegration> =
                        match WorkerPluginIntegration::load(&package_root, manifest.clone(), worker)
                        {
                            Ok(integration) => Arc::new(integration),
                            Err(err) => {
                                load_errors.push(format!(
                                    "failed to load plugin '{}': {err}",
                                    descriptor.plugin_id
                                ));
                                continue;
                            }
                        };

                    if by_id.contains_key(integration.id()) {
                        load_errors.push(format!(
                            "duplicate active integration id '{}'",
                            integration.id()
                        ));
                        continue;
                    }

                    if by_implant_type.contains_key(integration.implant_type()) {
                        load_errors.push(format!(
                            "duplicate active implant type '{}' from plugin '{}'",
                            integration.implant_type(),
                            manifest.id
                        ));
                        continue;
                    }

                    by_implant_type
                        .insert(integration.implant_type().to_string(), integration.clone());
                    by_id.insert(integration.id().to_string(), integration);
                }
            }
            Err(err) => load_errors.push(format!("failed to enumerate active plugins: {err}")),
        }

        Self {
            by_id: Arc::new(by_id),
            by_implant_type: Arc::new(by_implant_type),
            load_errors: Arc::new(load_errors),
        }
    }

    /// Resolves an integration by its unique plugin/integration id.
    pub fn by_id(&self, id: &str) -> Result<Arc<dyn ImplantIntegration>> {
        self.by_id
            .get(id)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("Unknown integration '{id}'")))
    }

    /// Resolves the integration responsible for a specific implant type.
    pub fn by_implant_type(&self, implant_type: &str) -> Result<Arc<dyn ImplantIntegration>> {
        self.by_implant_type
            .get(implant_type)
            .cloned()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("No integration registered for implant type '{implant_type}'"),
                )
            })
    }

    /// Returns non-fatal plugin load errors captured during registry construction.
    pub fn load_errors(&self) -> &[String] {
        self.load_errors.as_ref()
    }
}
