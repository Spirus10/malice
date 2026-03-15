use std::{
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct PayloadArtifact {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct PayloadRepository {
    search_roots: Vec<PathBuf>,
}

impl PayloadRepository {
    pub fn new() -> Self {
        Self {
            search_roots: vec![
                PathBuf::from("implant/zant/out/build/msvc-debug/payloads"),
                PathBuf::from("implant/zant/out/build/msvc-release/payloads"),
                PathBuf::from("implant/zant/out/build/default/payloads"),
            ],
        }
    }

    pub fn resolve(&self, logical_name: &str) -> Result<PayloadArtifact> {
        for root in &self.search_roots {
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
                "Payload '{logical_name}' was not found under {}",
                self.search_roots
                    .iter()
                    .map(|root| display_path(root))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ))
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
