use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod client;
pub mod handler;

pub use handler::WebSocketHandler;

/// Axum state wrapper for RDT
#[derive(Clone)]
pub struct RdtState {
    store: Arc<crate::document::DocumentStore>,
    clients: Arc<ClientManager>,
}

impl RdtState {
    /// Create a new RDT state
    pub fn new(store: Arc<crate::document::DocumentStore>) -> Self {
        Self {
            store,
            clients: Arc::new(ClientManager::new()),
        }
    }

    /// Get the document store
    pub fn store(&self) -> &Arc<crate::document::DocumentStore> {
        &self.store
    }

    /// Get the client manager
    pub fn clients(&self) -> &Arc<ClientManager> {
        &self.clients
    }
}

/// Manages connected WebSocket clients and their subscriptions
pub struct ClientManager {
    /// Map of client ID to client info
    clients: RwLock<HashMap<String, ClientInfo>>,
    /// Map of (document_id, map_key) to set of client IDs subscribed to it
    subscriptions: RwLock<HashMap<(String, String), Vec<String>>>,
}

/// Information about a connected client
pub struct ClientInfo {
    pub id: String,
    pub sender: tokio::sync::mpsc::UnboundedSender<crate::protocol::ServerMessage>,
}

impl ClientManager {
    /// Create a new client manager
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            subscriptions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new client
    pub async fn register_client(
        &self,
        sender: tokio::sync::mpsc::UnboundedSender<crate::protocol::ServerMessage>,
    ) -> String {
        let client_id = Uuid::new_v4().to_string();
        let client_info = ClientInfo {
            id: client_id.clone(),
            sender,
        };

        self.clients
            .write()
            .await
            .insert(client_id.clone(), client_info);
        tracing::info!("Registered client: {}", client_id);
        client_id
    }

    /// Unregister a client and clean up their subscriptions
    pub async fn unregister_client(&self, client_id: &str) {
        // Remove client
        self.clients.write().await.remove(client_id);

        // Clean up subscriptions
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.retain(|_, client_ids| {
            client_ids.retain(|id| id != client_id);
            !client_ids.is_empty()
        });

        tracing::info!("Unregistered client: {}", client_id);
    }

    /// Subscribe a client to a document map
    pub async fn subscribe_client(&self, client_id: &str, document_id: &str, map_key: &str) {
        let key = (document_id.to_string(), map_key.to_string());
        let mut subscriptions = self.subscriptions.write().await;

        subscriptions
            .entry(key)
            .or_insert_with(Vec::new)
            .push(client_id.to_string());

        tracing::debug!(
            "Client {} subscribed to document '{}', map '{}'",
            client_id,
            document_id,
            map_key
        );
    }

    /// Unsubscribe a client from a document map
    pub async fn unsubscribe_client(&self, client_id: &str, document_id: &str, map_key: &str) {
        let key = (document_id.to_string(), map_key.to_string());
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(client_ids) = subscriptions.get_mut(&key) {
            client_ids.retain(|id| id != client_id);
            if client_ids.is_empty() {
                subscriptions.remove(&key);
            }
        }

        tracing::debug!(
            "Client {} unsubscribed from document '{}', map '{}'",
            client_id,
            document_id,
            map_key
        );
    }

    /// Broadcast a message to all clients subscribed to a document map
    pub async fn broadcast_to_subscribers(
        &self,
        document_id: &str,
        map_key: &str,
        message: crate::protocol::ServerMessage,
    ) {
        let key = (document_id.to_string(), map_key.to_string());
        let subscriptions = self.subscriptions.read().await;

        if let Some(client_ids) = subscriptions.get(&key) {
            let clients = self.clients.read().await;

            // Use Arc to avoid cloning the message for each subscriber
            let message_arc = std::sync::Arc::new(message);

            for client_id in client_ids {
                if let Some(client_info) = clients.get(client_id) {
                    // Clone only the Arc, not the entire message
                    if client_info.sender.send((*message_arc).clone()).is_err() {
                        tracing::warn!("Failed to send message to client {}", client_id);
                    }
                }
            }
        }
    }

    /// Send a message to a specific client
    pub async fn send_to_client(&self, client_id: &str, message: crate::protocol::ServerMessage) {
        let clients = self.clients.read().await;
        if let Some(client_info) = clients.get(client_id) {
            if client_info.sender.send(message).is_err() {
                tracing::warn!("Failed to send message to client {}", client_id);
            }
        }
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a router with RDT WebSocket endpoint
pub fn router_with_rdt(store: Arc<crate::document::DocumentStore>) -> Router<RdtState> {
    let rdt_state = RdtState::new(store.clone());

    // Start the change forwarder to bridge DocumentStore changes to WebSocket clients
    start_change_forwarder(store, rdt_state.clients.clone());

    Router::new()
        .route("/rdt", get(websocket_handler))
        .with_state(rdt_state)
}

/// Create a router with RDT WebSocket endpoint using an existing RdtState
///
/// This is useful when you want to share the same RdtState across multiple parts
/// of your application to ensure WebSocket clients and change forwarders use the
/// same ClientManager instance.
pub fn router_with_rdt_state(rdt_state: RdtState) -> Router<RdtState> {
    // Start the change forwarder to bridge DocumentStore changes to WebSocket clients
    start_change_forwarder(rdt_state.store.clone(), rdt_state.clients.clone());

    Router::new()
        .route("/rdt", get(websocket_handler))
        .with_state(rdt_state)
}

/// Start a background task that forwards changes from the DocumentStore to WebSocket clients
fn start_change_forwarder(
    store: Arc<crate::document::DocumentStore>,
    client_manager: Arc<ClientManager>,
) {
    tokio::spawn(async move {
        let mut change_rx = store.subscribe_to_changes();

        tracing::info!("Started change forwarder for document store");

        loop {
            match change_rx.recv().await {
                Ok((document_id, map_key, change_event)) => {
                    tracing::debug!(
                        "Forwarding change for document '{}', map '{}': {:?}",
                        document_id,
                        map_key,
                        change_event
                    );

                    let server_message = match change_event {
                        crate::protocol::ChangeEvent::Single(change) => {
                            crate::protocol::ServerMessage::MapChange {
                                document_id: document_id.clone(),
                                map_key: map_key.clone(),
                                change,
                            }
                        }
                        crate::protocol::ChangeEvent::Batch(changes) => {
                            crate::protocol::ServerMessage::BatchMapChange {
                                document_id: document_id.clone(),
                                map_key: map_key.clone(),
                                changes,
                            }
                        }
                    };

                    client_manager
                        .broadcast_to_subscribers(&document_id, &map_key, server_message)
                        .await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        "Change forwarder lagged behind, skipped {} messages. Continuing...",
                        skipped
                    );
                    // Continue processing - we've skipped some messages but can keep going
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::error!(
                        "Change forwarder channel closed - DocumentStore sender dropped"
                    );
                    break;
                }
            }
        }

        tracing::warn!("Change forwarder ended");
    });
}

/// WebSocket handler endpoint
async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<RdtState>) -> Response {
    ws.on_upgrade(move |socket| WebSocketHandler::new(socket, state).handle())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentStore;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_change_forwarder_with_transactions() {
        // Create a document store
        let store = Arc::new(DocumentStore::new());

        // Create client manager and a mock client
        let client_manager = Arc::new(ClientManager::new());
        let (client_tx, mut client_rx) = mpsc::unbounded_channel();
        let client_id = client_manager.register_client(client_tx).await;

        // Subscribe the client to a document map
        client_manager
            .subscribe_client(&client_id, "test-doc", "test-map")
            .await;

        // Start the change forwarder
        start_change_forwarder(store.clone(), client_manager.clone());

        // Give the change forwarder a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Create a document and map
        let doc = store.create_document("test-doc".to_string());
        let map = doc.create_map("test-map".to_string());

        // Test 1: Non-transaction changes should send MapChange messages immediately
        map.insert("key1".to_string(), json!("value1"));

        // Should receive immediate MapChange
        let message = timeout(Duration::from_millis(100), client_rx.recv())
            .await
            .expect("Should receive message")
            .expect("Should have message");

        match message {
            crate::protocol::ServerMessage::MapChange {
                document_id,
                map_key,
                change,
            } => {
                assert_eq!(document_id, "test-doc");
                assert_eq!(map_key, "test-map");
                assert!(matches!(change, crate::protocol::Change::Insert { .. }));
            }
            _ => panic!("Expected MapChange message, got: {:?}", message),
        }

        // Test 2: Transaction changes should send BatchMapChange message on commit
        {
            let transaction = map.start_transaction().unwrap();

            // Make several changes
            map.insert("key2".to_string(), json!("value2"));
            map.insert("key3".to_string(), json!("value3"));
            map.remove("key2"); // This should cancel out key2 insert
            map.insert("key1".to_string(), json!("updated_value1")); // Update existing key

            // No messages should arrive yet (transaction is active)
            let result = timeout(Duration::from_millis(50), client_rx.recv()).await;
            assert!(
                result.is_err(),
                "Should not receive message during transaction"
            );

            // Commit transaction
            transaction.commit().unwrap();
        }

        // Should receive BatchMapChange with only net changes
        let message = timeout(Duration::from_millis(100), client_rx.recv())
            .await
            .expect("Should receive message")
            .expect("Should have message");

        match message {
            crate::protocol::ServerMessage::BatchMapChange {
                document_id,
                map_key,
                changes,
            } => {
                assert_eq!(document_id, "test-doc");
                assert_eq!(map_key, "test-map");
                assert_eq!(changes.len(), 2); // key3 insert + key1 update

                // Find the changes (order may vary due to HashMap iteration)
                let mut found_key3_insert = false;
                let mut found_key1_update = false;

                for change in changes {
                    match change {
                        crate::protocol::Change::Insert { key, value } if key == "key3" => {
                            assert_eq!(value, json!("value3"));
                            found_key3_insert = true;
                        }
                        crate::protocol::Change::Update {
                            key,
                            old_value,
                            new_value,
                        } if key == "key1" => {
                            assert_eq!(old_value, json!("value1"));
                            assert_eq!(new_value, json!("updated_value1"));
                            found_key1_update = true;
                        }
                        _ => panic!("Unexpected change: {:?}", change),
                    }
                }

                assert!(found_key3_insert, "Missing key3 insert");
                assert!(found_key1_update, "Missing key1 update");
            }
            _ => panic!("Expected BatchMapChange message, got: {:?}", message),
        }
    }
}
