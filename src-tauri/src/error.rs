use serde::Serialize;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("DB error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database not initialized")]
    NotInitialized,

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// Implement From<String> to easily convert string errors
impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::Other(err)
    }
}
