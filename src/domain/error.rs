use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid status transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("invalid status value: {0}")]
    InvalidStatus(String),

    #[error("invalid task type: {0}")]
    InvalidTaskType(String),

    #[error("{0}")]
    Message(String),
}

#[allow(dead_code)]
pub type DomainResult<T> = Result<T, DomainError>;
