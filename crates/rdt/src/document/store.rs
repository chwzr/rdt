use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};
use uuid::Uuid;

use super::{Document, DocumentHandle};
use crate::protocol::ChangeEvent;

/// Thread-safe store for managing documents
///
/// The DocumentStore is the main entry point for document management.
/// It provides thread-safe operations to create, access, and manage documents.
pub struct DocumentStore {
    documents: DashMap<String, Arc<Document>>,
    change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
}

impl DocumentStore {
    /// Create a new document store
    pub fn new() -> Self {
        let (change_tx, _) = broadcast::channel(1000);

        Self {
            documents: DashMap::new(),
            change_tx,
        }
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
}
