use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Subscribe to updates for a specific map in a document
    Subscribe {
        document_id: String,
        map_key: String,
    },
    /// Unsubscribe from updates for a specific map in a document
    Unsubscribe {
        document_id: String,
        map_key: String,
    },
    /// Request the full current state of a map
    GetFullState {
        document_id: String,
        map_key: String,
    },
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Full state of a map (sent in response to GetFullState or initial subscription)
    FullState {
        document_id: String,
        map_key: String,
        data: HashMap<String, JsonValue>,
    },
    /// A change occurred in a map (sent to all subscribers)
    MapChange {
        document_id: String,
        map_key: String,
        change: Change,
    },
    /// A batch of changes occurred in a map (sent to all subscribers)
    BatchMapChange {
        document_id: String,
        map_key: String,
        changes: Vec<Change>,
    },
    /// Error message
    Error { message: String },
    /// Acknowledgment of a request
    Ack { request_id: Option<String> },
}

/// Represents a change operation on a map
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Change {
    /// Insert a new key-value pair
    Insert { key: String, value: JsonValue },
    /// Update an existing key with a new value
    Update {
        key: String,
        old_value: JsonValue,
        new_value: JsonValue,
    },
    /// Remove a key-value pair
    Remove { key: String, old_value: JsonValue },
}

/// Internal enum to distinguish between single and batch changes for broadcasting
#[derive(Debug, Clone)]
pub enum ChangeEvent {
    Single(Change),
    Batch(Vec<Change>),
}

/// Encode a message using lib0 format
pub fn encode_message<T: Serialize>(message: &T) -> crate::RdtResult<Vec<u8>> {
    let json = serde_json::to_string(message)?;
    let mut encoder = Vec::new();
    lib0::encoding::Write::write_string(&mut encoder, &json);
    Ok(encoder)
}

/// Decode a message from lib0 format
pub fn decode_message<T: for<'de> Deserialize<'de>>(data: &[u8]) -> crate::RdtResult<T> {
    let mut decoder = lib0::decoding::Cursor::new(data);
    let json =
        lib0::decoding::Read::read_string(&mut decoder).map_err(|e| crate::RdtError::Protocol {
            message: format!("Failed to decode lib0 string: {}", e),
        })?;

    let message = serde_json::from_str(json)?;
    Ok(message)
}
