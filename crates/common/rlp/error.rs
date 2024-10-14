use thiserror::Error;

// TODO: improve errors
#[derive(Debug, Error)]
pub enum RLPDecodeError {
    #[error("InvalidLength")]
    InvalidLength,
    #[error("MalformedData")]
    MalformedData,
    #[error("MalformedBoolean")]
    MalformedBoolean,
    #[error("UnexpectedList")]
    UnexpectedList,
    #[error("UnexpectedString")]
    UnexpectedString,
    #[error("{0}")]
    Custom(String),
}

// TODO: improve errors
#[derive(Debug, Error)]
pub enum RLPEncodeError {
    #[error("InvalidCompression")]
    InvalidCompression,
    #[error("{0}")]
    Custom(String),
}
