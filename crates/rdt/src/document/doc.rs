use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, info};

use super::{DocumentMap, DocumentMapHandle};
use crate::protocol::ChangeEvent;

/// A document containing multiple named maps
///
/// Documents are containers for multiple maps, each identified by a string key.
/// They track when they've been modified for persistence purposes.
pub struct Document {
    id: String,
    maps: DashMap<String, Arc<DocumentMap>>,
    last_modified: Mutex<std::time::Instant>,
    dirty: AtomicBool,
    change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
}

impl Document {
    /// Create a new document with the given ID
    pub(crate) fn new(
        id: String,
        change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
    ) -> Self {
        Self {
            id,
            maps: DashMap::new(),
            last_modified: Mutex::new(std::time::Instant::now()),
            dirty: AtomicBool::new(false),
            change_tx,
        }
    }

    /// Get the document ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Create a new map with the given key
    ///
    /// If a map with the same key already exists, returns a handle to the existing map.
    pub fn create_map(&self, key: String) -> DocumentMapHandle {
        let map = self
            .maps
            .entry(key.clone())
            .or_insert_with(|| {
                info!("Creating new map '{}' in document '{}'", key, self.id);
                Arc::new(DocumentMap::new(
                    self.id.clone(),
                    key.clone(),
                    self.change_tx.clone(),
                ))
            })
            .clone();

        self.mark_dirty();
        DocumentMapHandle::new(map)
    }

    /// Get an existing map by key
    pub fn get_map(&self, key: &str) -> Option<DocumentMapHandle> {
        self.maps
            .get(key)
            .map(|entry| DocumentMapHandle::new(entry.value().clone()))
    }

    /// List all map keys in this document
    pub fn list_maps(&self) -> Vec<String> {
        self.maps.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Get the number of maps in this document
    pub fn map_count(&self) -> usize {
        self.maps.len()
    }

    /// Remove a map by key
    ///
    /// Returns true if the map was removed, false if it didn't exist.
    pub fn remove_map(&self, key: &str) -> bool {
        match self.maps.remove(key) {
            Some(_) => {
                info!("Removed map '{}' from document '{}'", key, self.id);
                self.mark_dirty();
                true
            }
            None => {
                debug!(
                    "Attempted to remove non-existent map '{}' from document '{}'",
                    key, self.id
                );
                false
            }
        }
    }

    /// Check if the document has been modified since last save
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    /// Mark the document as clean (typically called after saving)
    pub fn mark_clean(&self) {
        self.dirty.store(false, Ordering::Release);
        if let Ok(mut last_modified) = self.last_modified.lock() {
            *last_modified = std::time::Instant::now();
        }
    }

    /// Get the last modified time
    pub fn last_modified(&self) -> std::time::Instant {
        self.last_modified
            .lock()
            .map(|time| *time)
            .unwrap_or_else(|_| std::time::Instant::now())
    }

    /// Mark the document as dirty (modified)
    pub(crate) fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
        if let Ok(mut last_modified) = self.last_modified.lock() {
            *last_modified = std::time::Instant::now();
        }
    }

    /// Get all maps as a serializable format for persistence
    pub fn to_serializable(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut result = serde_json::Map::new();

        for entry in self.maps.iter() {
            let map_data = entry.value().to_serializable();
            result.insert(entry.key().clone(), serde_json::Value::Object(map_data));
        }

        result
    }

    /// Load maps from a serializable format (for loading from persistence)
    pub fn from_serializable(
        &self,
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::RdtResult<()> {
        for (map_key, map_value) in data {
            if let serde_json::Value::Object(map_data) = map_value {
                let map_handle = self.create_map(map_key.clone());
                map_handle.from_serializable(map_data)?;
            }
        }

        // Mark as clean since we just loaded from disk
        self.mark_clean();
        Ok(())
    }
}

/// A thread-safe handle to a document
///
/// DocumentHandle provides a convenient interface to work with documents
/// while ensuring thread safety through Arc.
#[derive(Clone)]
pub struct DocumentHandle {
    inner: Arc<Document>,
}

impl DocumentHandle {
    pub(crate) fn new(document: Arc<Document>) -> Self {
        Self { inner: document }
    }

    /// Get the document ID
    pub fn id(&self) -> &str {
        self.inner.id()
    }

    /// Create a new map with the given key
    pub fn create_map(&self, key: String) -> DocumentMapHandle {
        self.inner.create_map(key)
    }

    /// Get an existing map by key
    pub fn get_map(&self, key: &str) -> Option<DocumentMapHandle> {
        self.inner.get_map(key)
    }

    /// List all map keys in this document
    pub fn list_maps(&self) -> Vec<String> {
        self.inner.list_maps()
    }

    /// Get the number of maps in this document
    pub fn map_count(&self) -> usize {
        self.inner.map_count()
    }

    /// Remove a map by key
    pub fn remove_map(&self, key: &str) -> bool {
        self.inner.remove_map(key)
    }

    /// Check if the document has been modified since last save
    pub fn is_dirty(&self) -> bool {
        self.inner.is_dirty()
    }

    /// Mark the document as clean (typically called after saving)
    pub fn mark_clean(&self) {
        self.inner.mark_clean()
    }

    /// Get the last modified time
    pub fn last_modified(&self) -> std::time::Instant {
        self.inner.last_modified()
    }

    /// Get all maps as a serializable format for persistence
    pub fn to_serializable(&self) -> serde_json::Map<String, serde_json::Value> {
        self.inner.to_serializable()
    }

    /// Load maps from a serializable format (for loading from persistence)
    pub fn from_serializable(
        &self,
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::RdtResult<()> {
        self.inner.from_serializable(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    fn create_test_document() -> Document {
        let (tx, _) = broadcast::channel(10);
        Document::new("test-doc".to_string(), tx)
    }

    #[tokio::test]
    async fn test_document_creation() {
        let doc = create_test_document();
        assert_eq!(doc.id(), "test-doc");
        assert_eq!(doc.map_count(), 0);
        assert!(!doc.is_dirty()); // New documents start clean
    }

    #[tokio::test]
    async fn test_create_and_get_map() {
        let doc = create_test_document();

        let map_handle = doc.create_map("test-map".to_string());
        assert_eq!(map_handle.key(), "test-map");
        assert_eq!(doc.map_count(), 1);
        assert!(doc.is_dirty()); // Creating a map marks as dirty

        let retrieved = doc.get_map("test-map").unwrap();
        assert_eq!(retrieved.key(), "test-map");
    }

    #[tokio::test]
    async fn test_list_maps() {
        let doc = create_test_document();

        doc.create_map("map1".to_string());
        doc.create_map("map2".to_string());

        let mut maps = doc.list_maps();
        maps.sort();
        assert_eq!(maps, vec!["map1", "map2"]);
    }

    #[tokio::test]
    async fn test_remove_map() {
        let doc = create_test_document();

        doc.create_map("test-map".to_string());
        assert_eq!(doc.map_count(), 1);

        assert!(doc.remove_map("test-map"));
        assert_eq!(doc.map_count(), 0);

        // Removing non-existent map should return false
        assert!(!doc.remove_map("non-existent"));
    }

    #[tokio::test]
    async fn test_dirty_tracking() {
        let doc = create_test_document();
        assert!(!doc.is_dirty());

        doc.create_map("test-map".to_string());
        assert!(doc.is_dirty());

        doc.mark_clean();
        assert!(!doc.is_dirty());
    }
}
