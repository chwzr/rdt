use dashmap::{mapref, DashMap};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

use super::TransactionHandle;
use crate::protocol::{Change, ChangeEvent};

#[derive(Debug, Clone)]
enum TransactionState {
    /// Key was inserted (didn't exist before transaction)
    Inserted { value: JsonValue },
    /// Key was updated (existed before transaction)
    Updated {
        old_value: JsonValue,
        new_value: JsonValue,
    },
    /// Key was removed (existed before transaction)
    Removed { old_value: JsonValue },
}

/// A map within a document that stores key-value pairs
///
/// DocumentMap provides a DashMap-like interface for storing JSON values
/// and broadcasts changes to subscribers.
pub struct DocumentMap {
    data: DashMap<String, JsonValue>,
    document_id: String,
    map_key: String,
    change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
    // Transaction support
    transaction_active: AtomicBool,
    // Track final state of each key modified during transaction
    transaction_changes: Mutex<HashMap<String, TransactionState>>,
}

impl DocumentMap {
    /// Create a new document map
    pub(crate) fn new(
        document_id: String,
        map_key: String,
        change_tx: broadcast::Sender<(String, String, ChangeEvent)>,
    ) -> Self {
        Self {
            data: DashMap::new(),
            document_id,
            map_key,
            change_tx,
            transaction_active: AtomicBool::new(false),
            transaction_changes: Mutex::new(HashMap::new()),
        }
    }

    /// Get the map key
    pub fn key(&self) -> &str {
        &self.map_key
    }

    /// Get the document ID this map belongs to
    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    /// Insert a key-value pair into the map
    ///
    /// Returns the previous value if the key already existed.
    pub fn insert(&self, key: String, value: JsonValue) -> Option<JsonValue> {
        let old_value = self.data.insert(key.clone(), value.clone());

        if self.transaction_active.load(Ordering::Acquire) {
            // Transaction is active - update transaction state
            let mut changes = self.transaction_changes.lock().unwrap();

            let new_state = match changes.remove(&key) {
                Some(TransactionState::Inserted { .. }) => {
                    // Key was already inserted in this transaction, just update the value
                    TransactionState::Inserted { value }
                }
                Some(TransactionState::Updated { old_value, .. }) => {
                    // Key was already updated in this transaction, keep original old_value
                    TransactionState::Updated {
                        old_value,
                        new_value: value,
                    }
                }
                Some(TransactionState::Removed { old_value }) => {
                    // Key was removed then re-inserted, treat as update
                    TransactionState::Updated {
                        old_value,
                        new_value: value,
                    }
                }
                None => {
                    // First change to this key in transaction
                    if let Some(old) = &old_value {
                        TransactionState::Updated {
                            old_value: old.clone(),
                            new_value: value,
                        }
                    } else {
                        TransactionState::Inserted { value }
                    }
                }
            };

            changes.insert(key, new_state);
        } else {
            // No transaction - broadcast immediately
            let change = if let Some(old) = &old_value {
                Change::Update {
                    key: key.clone(),
                    old_value: old.clone(),
                    new_value: value,
                }
            } else {
                Change::Insert {
                    key: key.clone(),
                    value,
                }
            };
            self.broadcast_change_immediately(change);
        }

        old_value
    }

    /// Get a value by key
    ///
    /// Returns a reference to the value if it exists.
    pub fn get(&self, key: &str) -> Option<mapref::one::Ref<String, JsonValue>> {
        self.data.get(key)
    }

    /// Remove a key-value pair from the map
    ///
    /// Returns the removed value if the key existed.
    pub fn remove(&self, key: &str) -> Option<(String, JsonValue)> {
        if let Some((removed_key, removed_value)) = self.data.remove(key) {
            if self.transaction_active.load(Ordering::Acquire) {
                // Transaction is active - update transaction state
                let mut changes = self.transaction_changes.lock().unwrap();

                let new_state = match changes.remove(&removed_key) {
                    Some(TransactionState::Inserted { .. }) => {
                        // Key was inserted then removed in this transaction - net effect is nothing
                        None // Don't insert anything, net effect is no change
                    }
                    Some(TransactionState::Updated { old_value, .. }) => {
                        // Key was updated then removed - net effect is removal of original value
                        Some(TransactionState::Removed { old_value })
                    }
                    Some(TransactionState::Removed { old_value }) => {
                        // Already removed, shouldn't happen but keep the removal
                        Some(TransactionState::Removed { old_value })
                    }
                    None => {
                        // First change to this key in transaction is removal
                        Some(TransactionState::Removed {
                            old_value: removed_value.clone(),
                        })
                    }
                };

                if let Some(state) = new_state {
                    changes.insert(removed_key.clone(), state);
                }
            } else {
                // No transaction - broadcast immediately
                let change = Change::Remove {
                    key: removed_key.clone(),
                    old_value: removed_value.clone(),
                };
                self.broadcast_change_immediately(change);
            }

            Some((removed_key, removed_value))
        } else {
            None
        }
    }

    /// Get the number of key-value pairs in the map
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get an iterator over the key-value pairs
    pub fn iter(&self) -> dashmap::iter::Iter<String, JsonValue> {
        self.data.iter()
    }

    /// Check if a key exists in the map
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Clear all key-value pairs from the map
    pub fn clear(&self) {
        // We could broadcast individual Remove changes, but for performance
        // we'll just clear and let subscribers re-sync
        self.data.clear();
        debug!(
            "Cleared map '{}' in document '{}'",
            self.map_key, self.document_id
        );
    }

    /// Get all key-value pairs as a HashMap for full state sync
    pub fn to_hashmap(&self) -> std::collections::HashMap<String, JsonValue> {
        self.data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get all key-value pairs as a serializable format for persistence
    pub fn to_serializable(&self) -> serde_json::Map<String, JsonValue> {
        let mut result = serde_json::Map::new();
        for entry in self.data.iter() {
            result.insert(entry.key().clone(), entry.value().clone());
        }
        result
    }

    /// Load key-value pairs from a serializable format (for loading from persistence)
    pub fn from_serializable(
        &self,
        data: &serde_json::Map<String, JsonValue>,
    ) -> crate::RdtResult<()> {
        // Clear existing data first
        self.data.clear();

        // Insert all data without broadcasting changes (this is a bulk load)
        for (key, value) in data {
            self.data.insert(key.clone(), value.clone());
        }

        Ok(())
    }

    /// Broadcast a change immediately (used when no transaction is active)
    fn broadcast_change_immediately(&self, change: Change) {
        let message = (
            self.document_id.clone(),
            self.map_key.clone(),
            ChangeEvent::Single(change),
        );

        debug!(
            "Broadcasting single change to map '{}' in document '{}'",
            self.map_key, self.document_id
        );

        match self.change_tx.send(message) {
            Ok(receiver_count) => {
                debug!(
                    "Successfully sent change to {} receivers for map '{}' in document '{}'",
                    receiver_count, self.map_key, self.document_id
                );
            }
            Err(broadcast_error) => match broadcast_error {
                tokio::sync::broadcast::error::SendError(_) => {
                    debug!(
                        "No active receivers for changes to map '{}' in document '{}'",
                        self.map_key, self.document_id
                    );
                }
            },
        }
    }

    /// Start a transaction on this map
    pub(crate) fn start_transaction_internal(&self) -> crate::RdtResult<()> {
        if self.transaction_active.swap(true, Ordering::AcqRel) {
            return Err(crate::RdtError::TransactionAlreadyActive {
                map_key: self.map_key.clone(),
            });
        }

        // Clear any leftover transaction state
        let mut changes = self.transaction_changes.lock().unwrap();
        changes.clear();

        debug!(
            "Started transaction on map '{}' in document '{}'",
            self.map_key, self.document_id
        );

        Ok(())
    }

    /// Commit the transaction and send all collected changes as a batch
    pub(crate) fn commit_transaction_internal(&self) -> crate::RdtResult<()> {
        if !self.transaction_active.swap(false, Ordering::AcqRel) {
            return Err(crate::RdtError::NoActiveTransaction {
                map_key: self.map_key.clone(),
            });
        }

        let mut transaction_changes = self.transaction_changes.lock().unwrap();
        let changes_map = std::mem::take(&mut *transaction_changes);

        if !changes_map.is_empty() {
            // Convert transaction states to changes
            let changes: Vec<Change> = changes_map
                .into_iter()
                .map(|(key, state)| match state {
                    TransactionState::Inserted { value } => Change::Insert { key, value },
                    TransactionState::Updated {
                        old_value,
                        new_value,
                    } => Change::Update {
                        key,
                        old_value,
                        new_value,
                    },
                    TransactionState::Removed { old_value } => Change::Remove { key, old_value },
                })
                .collect();

            let changes_count = changes.len();

            // Send as batch
            let message = (
                self.document_id.clone(),
                self.map_key.clone(),
                ChangeEvent::Batch(changes),
            );

            match self.change_tx.send(message) {
                Ok(receiver_count) => {
                    debug!(
                        "Committed transaction with {} final changes to {} receivers on map '{}' in document '{}'",
                        changes_count, receiver_count, self.map_key, self.document_id
                    );
                }
                Err(_) => {
                    debug!(
                        "No subscribers for batch changes to map '{}' in document '{}'",
                        self.map_key, self.document_id
                    );
                }
            }
        } else {
            debug!(
                "Committed empty transaction on map '{}' in document '{}'",
                self.map_key, self.document_id
            );
        }

        Ok(())
    }

    /// Check if a transaction is currently active
    pub(crate) fn has_active_transaction(&self) -> bool {
        self.transaction_active.load(Ordering::Acquire)
    }
}

/// A thread-safe handle to a document map
///
/// DocumentMapHandle provides a convenient interface to work with maps
/// while ensuring thread safety through Arc.
#[derive(Clone)]
pub struct DocumentMapHandle {
    inner: Arc<DocumentMap>,
}

impl DocumentMapHandle {
    pub(crate) fn new(map: Arc<DocumentMap>) -> Self {
        Self { inner: map }
    }

    /// Get the map key
    pub fn key(&self) -> &str {
        self.inner.key()
    }

    /// Get the document ID this map belongs to
    pub fn document_id(&self) -> &str {
        self.inner.document_id()
    }

    /// Insert a key-value pair into the map
    pub fn insert(&self, key: String, value: JsonValue) -> Option<JsonValue> {
        self.inner.insert(key, value)
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Option<mapref::one::Ref<String, JsonValue>> {
        self.inner.get(key)
    }

    /// Remove a key-value pair from the map
    pub fn remove(&self, key: &str) -> Option<(String, JsonValue)> {
        self.inner.remove(key)
    }

    /// Get the number of key-value pairs in the map
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get an iterator over the key-value pairs
    pub fn iter(&self) -> dashmap::iter::Iter<String, JsonValue> {
        self.inner.iter()
    }

    /// Check if a key exists in the map
    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// Clear all key-value pairs from the map
    pub fn clear(&self) {
        self.inner.clear()
    }

    /// Get all key-value pairs as a HashMap for full state sync
    pub fn to_hashmap(&self) -> std::collections::HashMap<String, JsonValue> {
        self.inner.to_hashmap()
    }

    /// Get all key-value pairs as a serializable format for persistence
    pub fn to_serializable(&self) -> serde_json::Map<String, JsonValue> {
        self.inner.to_serializable()
    }

    /// Load key-value pairs from a serializable format
    pub fn from_serializable(
        &self,
        data: &serde_json::Map<String, JsonValue>,
    ) -> crate::RdtResult<()> {
        self.inner.from_serializable(data)
    }

    /// Start a new transaction on this map
    ///
    /// Only one transaction can be active at a time per map.
    /// The returned TransactionHandle will automatically commit when dropped.
    pub fn start_transaction(&self) -> crate::RdtResult<TransactionHandle> {
        self.inner.start_transaction_internal()?;
        Ok(TransactionHandle::new(self.clone()))
    }

    /// Check if a transaction is currently active on this map
    pub fn has_active_transaction(&self) -> bool {
        self.inner.has_active_transaction()
    }

    /// Internal method for committing transactions (used by TransactionHandle)
    pub(crate) fn commit_transaction_internal(&self) -> crate::RdtResult<()> {
        self.inner.commit_transaction_internal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::broadcast;

    fn create_test_map() -> DocumentMap {
        let (tx, _) = broadcast::channel(10);
        DocumentMap::new("test-doc".to_string(), "test-map".to_string(), tx)
    }

    #[tokio::test]
    async fn test_map_creation() {
        let map = create_test_map();
        assert_eq!(map.key(), "test-map");
        assert_eq!(map.document_id(), "test-doc");
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let map = create_test_map();

        let old_value = map.insert("key1".to_string(), json!("value1"));
        assert!(old_value.is_none());
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());

        let value = map.get("key1").unwrap();
        assert_eq!(*value, json!("value1"));
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let map = create_test_map();

        map.insert("key1".to_string(), json!("value1"));
        let old_value = map.insert("key1".to_string(), json!("value2"));

        assert_eq!(old_value, Some(json!("value1")));
        assert_eq!(map.len(), 1);

        let value = map.get("key1").unwrap();
        assert_eq!(*value, json!("value2"));
    }

    #[tokio::test]
    async fn test_remove() {
        let map = create_test_map();

        map.insert("key1".to_string(), json!("value1"));
        assert_eq!(map.len(), 1);

        let removed = map.remove("key1").unwrap();
        assert_eq!(removed, ("key1".to_string(), json!("value1")));
        assert_eq!(map.len(), 0);

        // Removing non-existent key should return None
        assert!(map.remove("non-existent").is_none());
    }

    #[tokio::test]
    async fn test_contains_key() {
        let map = create_test_map();

        assert!(!map.contains_key("key1"));
        map.insert("key1".to_string(), json!("value1"));
        assert!(map.contains_key("key1"));
    }

    #[tokio::test]
    async fn test_clear() {
        let map = create_test_map();

        map.insert("key1".to_string(), json!("value1"));
        map.insert("key2".to_string(), json!("value2"));
        assert_eq!(map.len(), 2);

        map.clear();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn test_to_hashmap() {
        let map = create_test_map();

        map.insert("key1".to_string(), json!("value1"));
        map.insert("key2".to_string(), json!(42));

        let hashmap = map.to_hashmap();
        assert_eq!(hashmap.len(), 2);
        assert_eq!(hashmap.get("key1"), Some(&json!("value1")));
        assert_eq!(hashmap.get("key2"), Some(&json!(42)));
    }

    #[tokio::test]
    async fn test_serialization() {
        let map = create_test_map();

        map.insert("key1".to_string(), json!("value1"));
        map.insert("key2".to_string(), json!({"nested": "object"}));

        let serialized = map.to_serializable();
        assert_eq!(serialized.len(), 2);

        let new_map = create_test_map();
        new_map.from_serializable(&serialized).unwrap();

        assert_eq!(new_map.len(), 2);
        assert_eq!(new_map.get("key1").unwrap().clone(), json!("value1"));
        assert_eq!(
            new_map.get("key2").unwrap().clone(),
            json!({"nested": "object"})
        );
    }

    #[tokio::test]
    async fn test_transaction_basic() {
        let (tx, mut rx) = broadcast::channel(10);
        let map = DocumentMap::new("test-doc".to_string(), "test-map".to_string(), tx);
        let map_handle = DocumentMapHandle::new(Arc::new(map));

        // Start a transaction
        let transaction = map_handle.start_transaction().unwrap();
        assert!(map_handle.has_active_transaction());
        assert!(transaction.is_active());

        // Make changes during transaction - these should be collected, not broadcast
        map_handle.insert("key1".to_string(), json!("value1"));
        map_handle.insert("key2".to_string(), json!("value2"));
        map_handle.remove("key1"); // This cancels out the key1 insert

        // No messages should have been sent yet
        assert!(rx.try_recv().is_err());

        // Commit the transaction
        transaction.commit().unwrap();
        assert!(!map_handle.has_active_transaction());

        // Now we should receive a batch message
        let (doc_id, map_key, change_event) = rx.recv().await.unwrap();
        assert_eq!(doc_id, "test-doc");
        assert_eq!(map_key, "test-map");

        match change_event {
            ChangeEvent::Batch(changes) => {
                // Only key2 insert should remain (key1 insert+remove cancels out)
                assert_eq!(changes.len(), 1);
                match &changes[0] {
                    Change::Insert { key, value } => {
                        assert_eq!(key, "key2");
                        assert_eq!(value, &json!("value2"));
                    }
                    _ => panic!("Expected Insert change for key2"),
                }
            }
            _ => panic!("Expected batch change event"),
        }
    }

    #[tokio::test]
    async fn test_transaction_auto_commit() {
        let (tx, mut rx) = broadcast::channel(10);
        let map = DocumentMap::new("test-doc".to_string(), "test-map".to_string(), tx);
        let map_handle = DocumentMapHandle::new(Arc::new(map));

        {
            // Start a transaction in a scope
            let _transaction = map_handle.start_transaction().unwrap();
            assert!(map_handle.has_active_transaction());

            // Make a change
            map_handle.insert("key1".to_string(), json!("value1"));

            // Transaction will auto-commit when it goes out of scope
        }

        // Transaction should be auto-committed
        assert!(!map_handle.has_active_transaction());

        // Should receive the batch message
        let (_, _, change_event) = rx.recv().await.unwrap();
        match change_event {
            ChangeEvent::Batch(changes) => {
                assert_eq!(changes.len(), 1);
            }
            _ => panic!("Expected batch change event"),
        }
    }

    #[tokio::test]
    async fn test_no_transaction_immediate_broadcast() {
        let (tx, mut rx) = broadcast::channel(10);
        let map = DocumentMap::new("test-doc".to_string(), "test-map".to_string(), tx);
        let map_handle = DocumentMapHandle::new(Arc::new(map));

        // Make changes without a transaction - should broadcast immediately
        map_handle.insert("key1".to_string(), json!("value1"));

        // Should receive immediate single change message
        let (_, _, change_event) = rx.recv().await.unwrap();
        match change_event {
            ChangeEvent::Single(change) => {
                assert!(matches!(change, Change::Insert { .. }));
            }
            _ => panic!("Expected single change event"),
        }
    }
}
