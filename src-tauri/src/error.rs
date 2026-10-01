use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication required")]
    AuthRequired,
    #[error("OAuth error: {0}")]
    OAuth(String),
    #[error("Google API error: {0}")]
    Google(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Invalid configuration: {0}")]
    Config(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Google sync token expired")]
    SyncTokenExpired,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
