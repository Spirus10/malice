#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplantFamily {
    CoffLoader,
    Unknown(String),
}

impl ImplantFamily {
    pub fn key(&self) -> &str {
        match self {
            Self::CoffLoader => "coff_loader",
            Self::Unknown(other) => other.as_str(),
        }
    }
}
