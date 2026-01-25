//! Traits for QMDB store operations.

/// Trait for reading values from a QMDB store.
pub trait QmdbGettable {
    /// The key type for lookups.
    type Key;
    /// The value type returned from lookups.
    type Value;
    /// Error type for operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Get a value by key, returning None if not found.
    fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error>;
}

/// Trait for batching writes to a QMDB store.
pub trait QmdbBatchable: QmdbGettable {
    /// Write a batch of key-value pairs. None values indicate deletion.
    fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (Self::Key, Option<Self::Value>)>;
}
