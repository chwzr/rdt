use super::DocumentMapHandle;
use crate::RdtResult;
use std::sync::atomic::{AtomicBool, Ordering};

/// A handle for managing a transaction on a document map
///
/// The transaction will automatically commit when the handle is dropped,
/// but can also be explicitly committed.
pub struct TransactionHandle {
    map_handle: DocumentMapHandle,
    committed: AtomicBool,
}

impl TransactionHandle {
    pub(crate) fn new(map_handle: DocumentMapHandle) -> Self {
        Self {
            map_handle,
            committed: AtomicBool::new(false),
        }
    }

    /// Commit the transaction, sending all collected changes as a batch
    pub fn commit(self) -> RdtResult<()> {
        if self.committed.swap(true, Ordering::AcqRel) {
            // Already committed
            return Ok(());
        }

        self.map_handle.commit_transaction_internal()
    }

    /// Check if the transaction is still active (not yet committed)
    pub fn is_active(&self) -> bool {
        !self.committed.load(Ordering::Acquire)
    }
}

impl Drop for TransactionHandle {
    fn drop(&mut self) {
        // Auto-commit if not already committed
        if !self.committed.swap(true, Ordering::AcqRel) {
            if let Err(e) = self.map_handle.commit_transaction_internal() {
                tracing::warn!("Failed to auto-commit transaction: {}", e);
            }
        }
    }
}
