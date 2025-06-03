//! # RDT - Real-time Document Transport
//!
//! A state synchronization framework for Axum-based Rust servers and frontends.
//!
//! RDT provides thread-safe document and map management with WebSocket-based synchronization
//! for real-time collaboration and state management.

pub mod document;
pub mod error;
pub mod protocol;

#[cfg(feature = "persistence")]
pub mod persistence;

#[cfg(feature = "axum")]
pub mod axum;

// Re-exports for convenience
pub use document::{
    Document, DocumentHandle, DocumentMap, DocumentMapHandle, DocumentStore, TransactionHandle,
};
pub use error::{RdtError, RdtResult};
pub use protocol::{Change, ChangeEvent, ClientMessage, ServerMessage};

#[cfg(feature = "axum")]
pub use axum::{router_with_rdt, router_with_rdt_state, RdtState, WebSocketHandler};

#[cfg(feature = "persistence")]
pub use persistence::PersistenceManager;
