use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlogError {
    #[error("{field:?}: {message:?}")]
    Input { field: String, message: String },
    #[error("{0}")]
    Recoverable(String),
}
