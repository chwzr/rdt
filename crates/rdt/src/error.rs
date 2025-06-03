use thiserror::Error;

/// Result type for RDT operations
pub type RdtResult<T> = Result<T, RdtError>;

/// Errors that can occur in RDT operations
#[derive(Error, Debug)]
pub enum RdtError {
    #[error("Document not found: {id}")]
    DocumentNotFound { id: String },

    #[error("Map not found: {key} in document {document_id}")]
    MapNotFound { document_id: String, key: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Protocol error: {message}")]
    Protocol { message: String },

    #[cfg(feature = "persistence")]
    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Transaction already active on map: {map_key}")]
    TransactionAlreadyActive { map_key: String },

    #[error("No active transaction on map: {map_key}")]
    NoActiveTransaction { map_key: String },
}
