use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
    path::PathBuf,
    sync::Arc,
};

use super::{types::ImplantIntegration, zant::ZantIntegration};

#[derive(Clone)]
pub struct ImplantIntegrationRegistry {
    by_id: Arc<HashMap<String, Arc<dyn ImplantIntegration>>>,
    by_implant_type: Arc<HashMap<String, Arc<dyn ImplantIntegration>>>,
}

impl ImplantIntegrationRegistry {
    pub fn new() -> Self {
        let integrations: Vec<Arc<dyn ImplantIntegration>> = vec![Arc::new(
            ZantIntegration::load(
                &PathBuf::from("integrations")
                    .join("zant")
                    .join("manifest.json"),
            )
            .unwrap_or_else(|err| panic!("failed to load zant integration manifest: {err}")),
        )];
        let mut by_id = HashMap::new();
        let mut by_implant_type = HashMap::new();

        for integration in integrations {
            by_implant_type.insert(integration.implant_type().to_string(), integration.clone());
            by_id.insert(integration.id().to_string(), integration);
        }

        Self {
            by_id: Arc::new(by_id),
            by_implant_type: Arc::new(by_implant_type),
        }
    }

    pub fn by_id(&self, id: &str) -> Result<Arc<dyn ImplantIntegration>> {
        self.by_id
            .get(id)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("Unknown integration '{id}'")))
    }

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
}
