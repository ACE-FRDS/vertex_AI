use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("invalid memory request: {0}")]
    Invalid(String),
    #[error("memory record not found")]
    NotFound,
    #[error("memory record version conflict")]
    Conflict,
    #[error("memory store is unavailable")]
    Unavailable,
}

impl From<sqlx::Error> for MemoryError {
    fn from(_error: sqlx::Error) -> Self {
        // SQL details may contain content or infrastructure information.
        Self::Unavailable
    }
}

impl From<sqlx::migrate::MigrateError> for MemoryError {
    fn from(_error: sqlx::migrate::MigrateError) -> Self {
        Self::Unavailable
    }
}
