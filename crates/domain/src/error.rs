use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl DomainError {
    pub fn validation<T: Into<String>>(message: T) -> Self {
        Self::Validation(message.into())
    }
}
