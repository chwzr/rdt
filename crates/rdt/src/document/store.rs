use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{Document, DocumentHandle};
use crate::protocol::ChangeEvent;

/// Thread-safe store for managing documents
///
/// The DocumentStore is the main entry point for document management.
/// It provides thread-safe operations to create, access, and manage documents.
pub struct DocumentStore {
    documents: Arc<DashMap<String, Arc<Document>>>,
    change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
    // Background task that listens for changes and marks documents dirty
    _dirty_marker_task: JoinHandle<()>,
}

impl DocumentStore {
    /// Create a new document store
    pub fn new() -> Self {
        let (change_tx, _) = broadcast::channel(1000);

        let documents = Arc::new(DashMap::new());

        let mut store = Self {
            documents: documents.clone(),
            change_tx: change_tx.clone(),
            _dirty_marker_task: tokio::spawn(async {}), // Placeholder, will be replaced
        };

        // Start the dirty marker background task with access to the shared documents
        let dirty_marker_task = Self::start_dirty_marker_task(change_tx.subscribe(), documents);

        // Replace the placeholder task
        store._dirty_marker_task = dirty_marker_task;

        store
    }

    /// Start a background task that listens for changes and marks documents dirty
    fn start_dirty_marker_task(
        mut change_rx: broadcast::Receiver<(String, String, ChangeEvent)>,
        documents: Arc<DashMap<String, Arc<Document>>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            info!("Started dirty marker task for document store");

            loop {
                match change_rx.recv().await {
                    Ok((document_id, map_key, change_event)) => {
                        debug!(
                            "Dirty marker task received change for document '{}', map '{}': {:?}",
                            document_id, map_key, change_event
                        );

                        // Find the document and mark it dirty
                        if let Some(document_entry) = documents.get(&document_id) {
                            document_entry.mark_dirty();
                            debug!("Successfully marked document '{}' as dirty", document_id);
                        } else {
                            debug!(
                                "Document '{}' not found when trying to mark dirty - it may have been removed. Available documents: {:?}",
                                document_id,
                                documents.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>()
                            );
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "Dirty marker task lagged behind, skipped {} messages",
                            skipped
                        );
                        // Continue processing - we might have missed some changes but can keep going
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Change channel closed, stopping dirty marker task");
                        break;
                    }
                }
            }

            info!("Dirty marker task ended");
        })
    }

    /// Create a new document with the given ID
    ///
    /// If a document with the same ID already exists, returns a handle to the existing document.
    pub fn create_document(&self, id: String) -> DocumentHandle {
        let document = self
            .documents
            .entry(id.clone())
            .or_insert_with(|| {
                info!("Creating new document: {}", id);
                Arc::new(Document::new(id.clone(), self.change_tx.clone()))
            })
            .clone();

        DocumentHandle::new(document)
    }

    /// Create a new document with a random UUID as ID
    pub fn create_document_with_uuid(&self) -> DocumentHandle {
        let id = Uuid::new_v4().to_string();
        self.create_document(id)
    }

    /// Get an existing document by ID
    pub fn get_document(&self, id: &str) -> Option<DocumentHandle> {
        self.documents
            .get(id)
            .map(|entry| DocumentHandle::new(entry.value().clone()))
    }

    /// List all document IDs
    pub fn list_documents(&self) -> Vec<String> {
        self.documents
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get the number of documents
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Remove a document by ID
    ///
    /// Returns true if the document was removed, false if it didn't exist.
    pub fn remove_document(&self, id: &str) -> bool {
        match self.documents.remove(id) {
            Some(_) => {
                info!("Removed document: {}", id);
                true
            }
            None => {
                debug!("Attempted to remove non-existent document: {}", id);
                false
            }
        }
    }

    /// Subscribe to changes across all documents
    ///
    /// Returns a receiver that will get notified of all changes.
    /// The tuple contains (document_id, map_key, change_event).
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<(String, String, ChangeEvent)> {
        self.change_tx.subscribe()
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_document_store_creation() {
        let store = DocumentStore::new();
        assert_eq!(store.document_count(), 0);
    }

    #[tokio::test]
    async fn test_create_and_get_document() {
        let store = DocumentStore::new();

        let doc_handle = store.create_document("test-doc".to_string());
        assert_eq!(doc_handle.id(), "test-doc");
        assert_eq!(store.document_count(), 1);

        let retrieved = store.get_document("test-doc").unwrap();
        assert_eq!(retrieved.id(), "test-doc");
    }

    #[tokio::test]
    async fn test_create_document_with_uuid() {
        let store = DocumentStore::new();

        let doc_handle = store.create_document_with_uuid();
        assert!(!doc_handle.id().is_empty());
        assert_eq!(store.document_count(), 1);
    }

    #[tokio::test]
    async fn test_list_documents() {
        let store = DocumentStore::new();

        store.create_document("doc1".to_string());
        store.create_document("doc2".to_string());

        let mut docs = store.list_documents();
        docs.sort();
        assert_eq!(docs, vec!["doc1", "doc2"]);
    }

    #[tokio::test]
    async fn test_remove_document() {
        let store = DocumentStore::new();

        store.create_document("test-doc".to_string());
        assert_eq!(store.document_count(), 1);

        assert!(store.remove_document("test-doc"));
        assert_eq!(store.document_count(), 0);

        // Removing non-existent document should return false
        assert!(!store.remove_document("non-existent"));
    }

    #[tokio::test]
    async fn test_map_insert_marks_document_dirty() {
        let store = DocumentStore::new();
        let doc_handle = store.create_document("test-doc".to_string());

        // Creating a map marks it dirty, so mark it clean first
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        let map_handle = doc_handle.create_map("test-map".to_string());

        // Creating the map should mark it dirty again
        assert!(doc_handle.is_dirty());
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        // Insert a value - this should mark the document dirty
        map_handle.insert("key1".to_string(), json!("value1"));

        // Give the background task a moment to process the change
        sleep(Duration::from_millis(10)).await;

        assert!(
            doc_handle.is_dirty(),
            "Document should be dirty after map insert"
        );
    }

    #[tokio::test]
    async fn test_map_remove_marks_document_dirty() {
        let store = DocumentStore::new();
        let doc_handle = store.create_document("test-doc".to_string());
        let map_handle = doc_handle.create_map("test-map".to_string());

        // Insert initial data
        map_handle.insert("key1".to_string(), json!("value1"));

        // Mark clean and verify
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        // Remove a value - this should mark the document dirty
        map_handle.remove("key1");

        // Give the background task a moment to process the change
        sleep(Duration::from_millis(10)).await;

        assert!(
            doc_handle.is_dirty(),
            "Document should be dirty after map remove"
        );
    }

    #[tokio::test]
    async fn test_map_clear_marks_document_dirty() {
        let store = DocumentStore::new();
        let doc_handle = store.create_document("test-doc".to_string());
        let map_handle = doc_handle.create_map("test-map".to_string());

        // Insert initial data
        map_handle.insert("key1".to_string(), json!("value1"));
        map_handle.insert("key2".to_string(), json!("value2"));

        // Mark clean and verify
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        // Clear the map - this should mark the document dirty
        map_handle.clear();

        // Give the background task a moment to process the change
        sleep(Duration::from_millis(10)).await;

        assert!(
            doc_handle.is_dirty(),
            "Document should be dirty after map clear"
        );
    }

    #[tokio::test]
    async fn test_transaction_commit_marks_document_dirty() {
        let store = DocumentStore::new();
        let doc_handle = store.create_document("test-doc".to_string());
        let map_handle = doc_handle.create_map("test-map".to_string());

        // Mark clean and verify
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        // Start a transaction and make changes
        {
            let transaction = map_handle.start_transaction().unwrap();
            map_handle.insert("key1".to_string(), json!("value1"));
            map_handle.insert("key2".to_string(), json!("value2"));

            // Document should still be clean during transaction
            assert!(
                !doc_handle.is_dirty(),
                "Document should be clean during transaction"
            );

            // Commit the transaction
            transaction.commit().unwrap();
        }

        // Give the background task a moment to process the batch change
        sleep(Duration::from_millis(10)).await;

        assert!(
            doc_handle.is_dirty(),
            "Document should be dirty after transaction commit"
        );
    }

    #[tokio::test]
    async fn test_empty_clear_does_not_mark_dirty() {
        let store = DocumentStore::new();
        let doc_handle = store.create_document("test-doc".to_string());
        let map_handle = doc_handle.create_map("test-map".to_string());

        // Mark clean and verify
        doc_handle.mark_clean();
        assert!(!doc_handle.is_dirty());

        // Clear an empty map - this should NOT mark the document dirty
        map_handle.clear();

        // Give the background task a moment to process (though nothing should be sent)
        sleep(Duration::from_millis(10)).await;

        assert!(
            !doc_handle.is_dirty(),
            "Document should remain clean after clearing empty map"
        );
    }

    #[tokio::test]
    async fn test_change_broadcast_system() {
        let store = DocumentStore::new();
        let mut change_rx = store.subscribe_to_changes();

        let doc_handle = store.create_document("test-doc".to_string());
        let map_handle = doc_handle.create_map("test-map".to_string());

        // Insert a value - this should generate a change event
        map_handle.insert("key1".to_string(), json!("value1"));

        // Try to receive the change event with a timeout
        let result = tokio::time::timeout(Duration::from_millis(100), change_rx.recv()).await;

        assert!(result.is_ok(), "Should receive change event");
        let (document_id, map_key, change_event) = result.unwrap().unwrap();
        assert_eq!(document_id, "test-doc");
        assert_eq!(map_key, "test-map");

        match change_event {
            crate::protocol::ChangeEvent::Single(change) => {
                assert!(matches!(change, crate::protocol::Change::Insert { .. }));
            }
            _ => panic!("Expected single change event"),
        }
    }
}
