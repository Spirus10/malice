use super::capabilities::ImplantCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplantFamily {
    CoffLoader,
    Unknown(String),
}

impl ImplantFamily {
    pub fn from_type(implant_type: &str) -> Self {
        match implant_type {
            "coff_loader" => Self::CoffLoader,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn capabilities(&self) -> Vec<ImplantCapability> {
        match self {
            Self::CoffLoader => vec![ImplantCapability::ExecuteCoff],
            Self::Unknown(_) => Vec::new(),
        }
    }
}
