use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::RdtState;
use crate::protocol::{decode_message, encode_message, ClientMessage, ServerMessage};

/// Handles WebSocket connections for individual clients
pub struct WebSocketHandler {
    socket: WebSocket,
    state: RdtState,
}

impl WebSocketHandler {
    /// Create a new WebSocket handler
    pub fn new(socket: WebSocket, state: RdtState) -> Self {
        Self { socket, state }
    }

    /// Handle the WebSocket connection
    pub async fn handle(self) {
        let (mut ws_sender, mut ws_receiver) = self.socket.split();

        // Create a channel for sending messages to this client
        let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

        // Register the client
        let client_id = self.state.clients().register_client(tx).await;
        let client_id_clone = client_id.clone();

        info!(
            "New WebSocket connection established for client {}",
            client_id
        );

        // Spawn task to send messages to the WebSocket
        let sender_task = {
            let client_id_for_sender = client_id.clone();
            tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    match encode_message(&message) {
                        Ok(encoded) => {
                            if let Err(e) = ws_sender.send(Message::Binary(encoded.into())).await {
                                error!(
                                    "Failed to send WebSocket message to client {}: {}",
                                    client_id_for_sender, e
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to encode message for client {}: {}",
                                client_id_for_sender, e
                            );
                        }
                    }
                }
                debug!("Sender task ended for client {}", client_id_for_sender);
            })
        };

        // Handle incoming messages from the WebSocket
        let receiver_task = {
            let state = self.state.clone();
            let client_id = client_id.clone();

            tokio::spawn(async move {
                while let Some(msg) = ws_receiver.next().await {
                    match msg {
                        Ok(Message::Binary(data)) => {
                            if let Err(e) = handle_client_message(&state, &client_id, &data).await {
                                error!("Error handling client message: {}", e);

                                let error_msg = ServerMessage::Error {
                                    message: format!("Error processing message: {}", e),
                                };
                                state.clients().send_to_client(&client_id, error_msg).await;
                            }
                        }
                        Ok(Message::Text(text)) => {
                            warn!(
                                "Received unexpected text message from client {}: {}",
                                client_id, text
                            );
                        }
                        Ok(Message::Close(_)) => {
                            info!("Client {} closed connection normally", client_id);
                            break;
                        }
                        Ok(Message::Ping(_data)) => {
                            debug!("Received ping from client {}", client_id);
                            // Axum handles pong responses automatically
                        }
                        Ok(Message::Pong(_)) => {
                            debug!("Received pong from client {}", client_id);
                        }
                        Err(e) => {
                            warn!("WebSocket error for client {}: {}", client_id, e);
                            break;
                        }
                    }
                }
                debug!("Receiver task ended for client {}", client_id);
            })
        };

        // Wait for either task to complete (indicating connection should close)
        let completion_reason = tokio::select! {
            _ = sender_task => {
                "sender task completed"
            }
            _ = receiver_task => {
                "receiver task completed"
            }
        };

        info!(
            "WebSocket connection ending for client {} ({})",
            client_id, completion_reason
        );

        // Clean up client registration
        self.state
            .clients()
            .unregister_client(&client_id_clone)
            .await;

        info!(
            "Client {} fully disconnected and cleaned up",
            client_id_clone
        );
    }
}

/// Handle a message from a client
async fn handle_client_message(
    state: &RdtState,
    client_id: &str,
    data: &[u8],
) -> crate::RdtResult<()> {
    let message: ClientMessage = decode_message(data)?;

    match message {
        ClientMessage::Subscribe {
            document_id,
            map_key,
        } => {
            debug!(
                "Client {} subscribing to document '{}', map '{}'",
                client_id, document_id, map_key
            );

            // Subscribe the client
            state
                .clients()
                .subscribe_client(client_id, &document_id, &map_key)
                .await;

            // Send current state if the map exists
            if let Some(doc_handle) = state.store().get_document(&document_id) {
                if let Some(map_handle) = doc_handle.get_map(&map_key) {
                    let full_state = ServerMessage::FullState {
                        document_id: document_id.clone(),
                        map_key: map_key.clone(),
                        data: map_handle.to_hashmap(),
                    };

                    state.clients().send_to_client(client_id, full_state).await;
                }
            }

            // Send acknowledgment
            let ack = ServerMessage::Ack { request_id: None };
            state.clients().send_to_client(client_id, ack).await;
        }

        ClientMessage::Unsubscribe {
            document_id,
            map_key,
        } => {
            debug!(
                "Client {} unsubscribing from document '{}', map '{}'",
                client_id, document_id, map_key
            );

            state
                .clients()
                .unsubscribe_client(client_id, &document_id, &map_key)
                .await;

            // Send acknowledgment
            let ack = ServerMessage::Ack { request_id: None };
            state.clients().send_to_client(client_id, ack).await;
        }

        ClientMessage::GetFullState {
            document_id,
            map_key,
        } => {
            debug!(
                "Client {} requesting full state for document '{}', map '{}'",
                client_id, document_id, map_key
            );

            let response = if let Some(doc_handle) = state.store().get_document(&document_id) {
                if let Some(map_handle) = doc_handle.get_map(&map_key) {
                    ServerMessage::FullState {
                        document_id: document_id.clone(),
                        map_key: map_key.clone(),
                        data: map_handle.to_hashmap(),
                    }
                } else {
                    ServerMessage::Error {
                        message: format!(
                            "Map '{}' not found in document '{}'",
                            map_key, document_id
                        ),
                    }
                }
            } else {
                ServerMessage::Error {
                    message: format!("Document '{}' not found", document_id),
                }
            };

            state.clients().send_to_client(client_id, response).await;
        }
    }

    Ok(())
}
