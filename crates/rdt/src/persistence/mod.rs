use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

use crate::document::DocumentStore;

/// Manages background persistence for all documents
pub struct PersistenceManager {
    store: Arc<DocumentStore>,
    storage_path: PathBuf,
    check_interval: Duration,
    handles: JoinSet<()>,
}

impl PersistenceManager {
    /// Create a new persistence manager
    pub fn new(store: Arc<DocumentStore>, storage_path: impl Into<PathBuf>) -> Self {
        Self {
            store,
            storage_path: storage_path.into(),
            check_interval: Duration::from_secs(10), // Default: check every 10 seconds
            handles: JoinSet::new(),
        }
    }

    /// Set the interval for checking dirty documents
    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    /// Start the persistence manager
    ///
    /// This will spawn worker tasks for each document and start monitoring for changes.
    pub async fn start(&mut self) -> crate::RdtResult<()> {
        // Ensure storage directory exists
        tokio::fs::create_dir_all(&self.storage_path).await?;

        info!(
            "Starting persistence manager with storage path: {:?}",
            self.storage_path
        );

        // Start the main persistence loop
        let store = self.store.clone();
        let storage_path = self.storage_path.clone();
        let check_interval = self.check_interval;

        self.handles.spawn(async move {
            let mut interval = interval(check_interval);

            loop {
                interval.tick().await;

                // Get all documents and check if they're dirty
                let document_ids = store.list_documents();

                for doc_id in document_ids {
                    if let Some(doc_handle) = store.get_document(&doc_id) {
                        if doc_handle.is_dirty() {
                            if let Err(e) = persist_document(&doc_handle, &storage_path).await {
                                error!("Failed to persist document '{}': {}", doc_id, e);
                            } else {
                                info!("Persisted document '{}'", doc_id);
                                doc_handle.mark_clean();
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the persistence manager
    ///
    /// This will cancel all running tasks and perform a final persistence pass.
    pub async fn stop(&mut self) {
        info!("Stopping persistence manager");

        // Perform final persistence pass
        let document_ids = self.store.list_documents();
        for doc_id in document_ids {
            if let Some(doc_handle) = self.store.get_document(&doc_id) {
                if doc_handle.is_dirty() {
                    if let Err(e) = persist_document(&doc_handle, &self.storage_path).await {
                        error!(
                            "Failed to persist document '{}' during shutdown: {}",
                            doc_id, e
                        );
                    } else {
                        info!("Final persist of document '{}'", doc_id);
                        doc_handle.mark_clean();
                    }
                }
            }
        }

        // Cancel all tasks
        self.handles.abort_all();

        // Wait for tasks to finish
        while let Some(result) = self.handles.join_next().await {
            if let Err(e) = result {
                if !e.is_cancelled() {
                    error!("Persistence task error: {}", e);
                }
            }
        }
    }

    /// Load all documents from storage
    ///
    /// This should be called on startup to restore persisted state.
    pub async fn load_all_documents(&self) -> crate::RdtResult<()> {
        info!(
            "Loading documents from storage path: {:?}",
            self.storage_path
        );

        let mut entries = tokio::fs::read_dir(&self.storage_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("rdt") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Err(e) = load_document(&self.store, file_stem, &path).await {
                        error!("Failed to load document from {:?}: {}", path, e);
                    } else {
                        info!("Loaded document '{}'", file_stem);
                    }
                }
            }
        }

        Ok(())
    }
}

/// Persist a single document to disk
async fn persist_document(
    doc_handle: &crate::document::DocumentHandle,
    storage_path: &Path,
) -> crate::RdtResult<()> {
    let doc_id = doc_handle.id();
    let file_path = storage_path.join(format!("{}.rdt", doc_id));

    let serialized_data = doc_handle.to_serializable();
    let json_data = serde_json::to_string_pretty(&serialized_data)?;

    tokio::fs::write(&file_path, json_data).await?;

    Ok(())
}

/// Load a single document from disk
async fn load_document(
    store: &DocumentStore,
    doc_id: &str,
    file_path: &PathBuf,
) -> crate::RdtResult<()> {
    let json_data = tokio::fs::read_to_string(file_path).await?;
    let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&json_data)?;

    let doc_handle = store.create_document(doc_id.to_string());
    doc_handle.from_serializable(&data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_persistence_manager_creation() {
        let store = Arc::new(DocumentStore::new());
        let temp_dir = TempDir::new().unwrap();

        let manager = PersistenceManager::new(store, temp_dir.path());
        assert_eq!(manager.check_interval, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_document_persistence() {
        let store = Arc::new(DocumentStore::new());
        let temp_dir = TempDir::new().unwrap();

        // Create a document with some data
        let doc = store.create_document("test-doc".to_string());
        let map = doc.create_map("test-map".to_string());
        map.insert("key1".to_string(), json!("value1"));
        map.insert("key2".to_string(), json!(42));

        // Persist the document
        persist_document(&doc, &temp_dir.path().to_path_buf())
            .await
            .unwrap();

        // Check that the file was created
        let file_path = temp_dir.path().join("test-doc.rdt");
        assert!(file_path.exists());

        // Load the document into a new store
        let new_store = Arc::new(DocumentStore::new());
        load_document(&new_store, "test-doc", &file_path)
            .await
            .unwrap();

        // Verify the data was loaded correctly
        let loaded_doc = new_store.get_document("test-doc").unwrap();
        let loaded_map = loaded_doc.get_map("test-map").unwrap();

        assert_eq!(loaded_map.len(), 2);
        assert_eq!(loaded_map.get("key1").unwrap().clone(), json!("value1"));
        assert_eq!(loaded_map.get("key2").unwrap().clone(), json!(42));
    }
}
