//! Filesystem-backed plugin package store.
//!
//! Directory layout:
//!
//!   plugins/
//!     packages/
//!       <plugin_id>/
//!         <version>/   <- immutable installed package copy
//!     active/
//!       <plugin_id>/   <- activated copy used at runtime
//!
//! Activation model:
//!
//!   install_from_dir(source)
//!      -> copy into packages/<id>/<version>
//!
//!   activate(id, version?)
//!      -> resolve installed version
//!      -> replace active/<id> with a copy of that package
//!
//! The active copy is what the integration registry loads, which keeps runtime
//! discovery separate from the archive of installed versions.

use std::{
    fs,
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::manifest::IntegrationManifest;

const SUPPORTED_PLUGIN_API_VERSION: u32 = 1;

fn validate_path_component(value: &str, field_name: &str) -> Result<()> {
    if value.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{field_name} must not be empty"),
        ));
    }
    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{field_name} contains invalid path characters"),
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{field_name} contains invalid characters"),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginPackageDescriptor {
    pub package_id: String,
    pub plugin_id: String,
    pub version: String,
    pub plugin_api_version: u32,
    pub runtime: PluginRuntimeDescriptor,
    pub manifest_path: String,
    pub min_core_version: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub artifact_roots: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginRuntimeDescriptor {
    #[serde(rename = "type")]
    pub runtime_type: String,
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstalledPluginSummary {
    pub plugin_id: String,
    pub version: String,
    pub active: bool,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginInspection {
    pub descriptor: PluginPackageDescriptor,
    pub manifest: IntegrationManifest,
    pub package_root: PathBuf,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct PluginStore {
    root: PathBuf,
}

impl PluginPackageDescriptor {
    /// Loads `plugin.json` from a package root.
    pub fn load(package_root: &Path) -> Result<Self> {
        let descriptor_path = package_root.join("plugin.json");
        let contents = fs::read_to_string(&descriptor_path).map_err(|err| {
            Error::new(
                err.kind(),
                format!(
                    "Unable to read plugin descriptor '{}': {err}",
                    descriptor_path.display()
                ),
            )
        })?;
        serde_json::from_str(&contents).map_err(|err| {
            Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Unable to parse plugin descriptor '{}': {err}",
                    descriptor_path.display()
                ),
            )
        })
    }

    /// Validates that the package can be installed and activated by this core.
    ///
    /// Checks include:
    ///
    ///   - semantic version syntax
    ///   - minimum supported core version
    ///   - supported plugin API version
    ///   - required manifest and runtime files exist
    ///   - descriptor fields agree with the manifest
    pub fn validate(&self, package_root: &Path) -> Result<()> {
        validate_path_component(&self.plugin_id, "plugin_id")?;
        validate_path_component(&self.version, "version")?;
        parse_version(&self.version)?;
        let min_core_version = parse_version(&self.min_core_version)?;
        let core_version =
            parse_version(env!("CARGO_PKG_VERSION")).expect("crate version should always be valid");
        if core_version < min_core_version {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "Plugin '{}' requires core version {} or newer",
                    self.plugin_id, self.min_core_version
                ),
            ));
        }
        if self.plugin_api_version != SUPPORTED_PLUGIN_API_VERSION {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "Plugin '{}' targets unsupported plugin API version {}",
                    self.plugin_id, self.plugin_api_version
                ),
            ));
        }
        let manifest_path = package_root.join(&self.manifest_path);
        if !manifest_path.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Plugin manifest '{}' is missing", manifest_path.display()),
            ));
        }
        if self.runtime.command.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Plugin '{}' runtime command is empty", self.plugin_id),
            ));
        }
        let runtime_entry = package_root.join(&self.runtime.command[0]);
        if !runtime_entry.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Plugin runtime entry '{}' is missing",
                    runtime_entry.display()
                ),
            ));
        }

        let manifest = self.load_manifest(package_root)?;
        if manifest.id != self.plugin_id {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Manifest id '{}' does not match plugin id '{}'",
                    manifest.id, self.plugin_id
                ),
            ));
        }
        if manifest.plugin_api_version != self.plugin_api_version {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Manifest plugin_api_version {} does not match descriptor version {}",
                    manifest.plugin_api_version, self.plugin_api_version
                ),
            ));
        }

        Ok(())
    }

    /// Loads the integration manifest referenced by the descriptor.
    pub fn load_manifest(&self, package_root: &Path) -> Result<IntegrationManifest> {
        IntegrationManifest::load(&package_root.join(&self.manifest_path))
    }
}

impl PluginStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Enumerates all installed packages and marks whichever version is active.
    pub fn list_installed(&self) -> Result<Vec<InstalledPluginSummary>> {
        self.ensure_layout()?;
        let mut installed = Vec::new();
        for plugin_dir in read_dirs(&self.packages_root())? {
            let plugin_id = file_name_string(&plugin_dir)?;
            let active_version = self.active_version(&plugin_id)?;
            for version_dir in read_dirs(&plugin_dir)? {
                let descriptor = PluginPackageDescriptor::load(&version_dir)?;
                installed.push(InstalledPluginSummary {
                    plugin_id: descriptor.plugin_id,
                    version: descriptor.version.clone(),
                    active: active_version
                        .as_deref()
                        .map(|version| version == descriptor.version)
                        .unwrap_or(false),
                    install_path: version_dir,
                });
            }
        }

        installed.sort_by(|left, right| {
            left.plugin_id
                .cmp(&right.plugin_id)
                .then_with(|| left.version.cmp(&right.version))
        });
        Ok(installed)
    }

    /// Installs a package by copying it into the versioned package archive.
    pub fn install_from_dir(&self, source: &Path) -> Result<InstalledPluginSummary> {
        self.ensure_layout()?;
        let descriptor = PluginPackageDescriptor::load(source)?;
        validate_path_component(&descriptor.plugin_id, "plugin_id")?;
        validate_path_component(&descriptor.version, "version")?;
        descriptor.validate(source)?;
        let destination = self.package_dir(&descriptor.plugin_id, &descriptor.version);
        if destination.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "Plugin '{}' version {} is already installed",
                    descriptor.plugin_id, descriptor.version
                ),
            ));
        }

        copy_dir_all(source, &destination)?;
        Ok(InstalledPluginSummary {
            plugin_id: descriptor.plugin_id,
            version: descriptor.version,
            active: false,
            install_path: destination,
        })
    }

    /// Activates a package version by copying it into the runtime `active` tree.
    ///
    /// The store intentionally activates via copy rather than pointer/symlink so
    /// the runtime loader always sees a self-contained directory tree.
    pub fn activate(
        &self,
        plugin_id: &str,
        version: Option<&str>,
    ) -> Result<InstalledPluginSummary> {
        self.ensure_layout()?;
        validate_path_component(plugin_id, "plugin_id")?;
        if let Some(v) = version {
            validate_path_component(v, "version")?;
        }
        let package_root = self.resolve_installed_package(plugin_id, version)?;
        let descriptor = PluginPackageDescriptor::load(&package_root)?;
        let active_root = self.active_plugin_root(plugin_id);
        remove_dir_if_exists(&active_root)?;
        copy_dir_all(&package_root, &active_root)?;
        Ok(InstalledPluginSummary {
            plugin_id: descriptor.plugin_id,
            version: descriptor.version,
            active: true,
            install_path: package_root,
        })
    }

    /// Removes the active runtime copy for a plugin.
    pub fn deactivate(&self, plugin_id: &str) -> Result<()> {
        self.ensure_layout()?;
        let active_root = self.active_plugin_root(plugin_id);
        if !active_root.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Plugin '{}' is not active", plugin_id),
            ));
        }
        remove_dir_if_exists(&active_root)
    }

    /// Deletes one archived plugin version when it is not currently active.
    pub fn remove(&self, plugin_id: &str, version: &str) -> Result<()> {
        self.ensure_layout()?;
        validate_path_component(plugin_id, "plugin_id")?;
        validate_path_component(version, "version")?;
        if self.active_version(plugin_id)?.as_deref() == Some(version) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Plugin '{}' version {} is active", plugin_id, version),
            ));
        }

        let package_root = self.package_dir(plugin_id, version);
        if !package_root.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Plugin '{}' version {} is not installed",
                    plugin_id, version
                ),
            ));
        }
        remove_dir_if_exists(&package_root)
    }

    /// Loads both descriptor and manifest details for an installed package.
    pub fn inspect(&self, plugin_id: &str, version: Option<&str>) -> Result<PluginInspection> {
        self.ensure_layout()?;
        validate_path_component(plugin_id, "plugin_id")?;
        if let Some(v) = version {
            validate_path_component(v, "version")?;
        }
        let package_root = self.resolve_installed_package(plugin_id, version)?;
        let descriptor = PluginPackageDescriptor::load(&package_root)?;
        let manifest = descriptor.load_manifest(&package_root)?;
        let active =
            self.active_version(plugin_id)?.as_deref() == Some(descriptor.version.as_str());
        Ok(PluginInspection {
            descriptor,
            manifest,
            package_root,
            active,
        })
    }

    /// Returns all currently active plugin roots that should be loaded at runtime.
    pub fn active_plugin_roots(&self) -> Result<Vec<PathBuf>> {
        self.ensure_layout()?;
        read_dirs(&self.active_root())
    }

    pub fn load_descriptor(&self, package_root: &Path) -> Result<PluginPackageDescriptor> {
        PluginPackageDescriptor::load(package_root)
    }

    /// Resolves a package path, defaulting to the highest installed version.
    ///
    /// Selection flow:
    ///
    ///   explicit version
    ///      -> packages/<id>/<version>
    ///
    ///   no version
    ///      -> enumerate installed versions
    ///      -> parse semantic versions
    ///      -> choose max(version)
    fn resolve_installed_package(&self, plugin_id: &str, version: Option<&str>) -> Result<PathBuf> {
        validate_path_component(plugin_id, "plugin_id")?;
        if let Some(v) = version {
            validate_path_component(v, "version")?;
        }
        let plugin_root = self.packages_root().join(plugin_id);
        if !plugin_root.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Plugin '{}' is not installed", plugin_id),
            ));
        }

        if let Some(version) = version {
            let package_root = plugin_root.join(version);
            if !package_root.exists() {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "Plugin '{}' version {} is not installed",
                        plugin_id, version
                    ),
                ));
            }
            return Ok(package_root);
        }

        let mut candidates = Vec::new();
        for version_dir in read_dirs(&plugin_root)? {
            let descriptor = PluginPackageDescriptor::load(&version_dir)?;
            let parsed = parse_version(&descriptor.version)?;
            candidates.push((parsed, version_dir));
        }

        candidates
            .into_iter()
            .max_by(|left, right| left.0.cmp(&right.0))
            .map(|(_, path)| path)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("Plugin '{}' has no installed versions", plugin_id),
                )
            })
    }

    /// Reads the version of the currently active runtime copy, if any.
    fn active_version(&self, plugin_id: &str) -> Result<Option<String>> {
        let active_root = self.active_plugin_root(plugin_id);
        if !active_root.exists() {
            return Ok(None);
        }
        Ok(Some(PluginPackageDescriptor::load(&active_root)?.version))
    }

    fn ensure_layout(&self) -> Result<()> {
        fs::create_dir_all(self.packages_root())?;
        fs::create_dir_all(self.active_root())
    }

    fn packages_root(&self) -> PathBuf {
        self.root.join("packages")
    }

    fn active_root(&self) -> PathBuf {
        self.root.join("active")
    }

    fn package_dir(&self, plugin_id: &str, version: &str) -> PathBuf {
        self.packages_root().join(plugin_id).join(version)
    }

    fn active_plugin_root(&self, plugin_id: &str) -> PathBuf {
        self.active_root().join(plugin_id)
    }
}

fn parse_version(input: &str) -> Result<(u64, u64, u64)> {
    let mut parts = input.split('.');
    let major = parse_version_part(parts.next(), input)?;
    let minor = parse_version_part(parts.next(), input)?;
    let patch = parse_version_part(parts.next(), input)?;
    if parts.next().is_some() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid version '{}'", input),
        ));
    }
    Ok((major, minor, patch))
}

/// Parses one component of a strict `major.minor.patch` version string.
fn parse_version_part(part: Option<&str>, original: &str) -> Result<u64> {
    part.ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Invalid version '{}'", original),
        )
    })?
    .parse::<u64>()
    .map_err(|_| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Invalid version '{}'", original),
        )
    })
}

/// Returns sorted child directories under `root`.
fn read_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    if !root.exists() {
        return Ok(dirs);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// Extracts the final path component as a UTF-8 string.
fn file_name_string(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                format!("Path '{}' has no valid file name", path.display()),
            )
        })
}

/// Recursively copies a directory tree.
fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

/// Deletes a directory tree when it exists.
fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
