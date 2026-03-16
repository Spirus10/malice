use std::{
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct PayloadArtifact {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub trait ArtifactSource {
    fn resolve(&self, logical_name: &str, search_roots: &[PathBuf]) -> Result<PayloadArtifact>;
}

#[derive(Clone, Default)]
pub struct PayloadRepository;

impl PayloadRepository {
    pub fn new() -> Self {
        Self
    }
}

impl ArtifactSource for PayloadRepository {
    fn resolve(&self, logical_name: &str, search_roots: &[PathBuf]) -> Result<PayloadArtifact> {
        for root in search_roots {
            let candidate = root.join(format!("{logical_name}.obj"));
            if candidate.exists() {
                return Ok(PayloadArtifact {
                    file_name: candidate
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("payload.obj")
                        .to_string(),
                    bytes: std::fs::read(&candidate)?,
                });
            }
        }

        Err(Error::new(
            ErrorKind::NotFound,
            format!(
                "Artifact '{logical_name}' was not found under {}",
                search_roots
                    .iter()
                    .map(|path| display_path(path))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ))
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
