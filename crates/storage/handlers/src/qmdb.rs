//! Thread-safe QMDB handle.

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use alloy_primitives::{Address, B256, U256};
use kora_qmdb::{
    AccountEncoding, AccountUpdate, ChangeSet, QmdbBatchable, QmdbGettable, QmdbStore, StorageKey,
};

use crate::error::HandleError;

/// Thread-safe handle to QMDB stores.
///
/// Wraps `QmdbStore` with `Arc<RwLock>` for safe concurrent access.
/// Implements REVM database traits via the `adapter` module.
pub struct QmdbHandle<A, S, C> {
    inner: Arc<RwLock<QmdbStore<A, S, C>>>,
}

impl<A, S, C> Clone for QmdbHandle<A, S, C> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<A, S, C> QmdbHandle<A, S, C> {
    /// Create a new handle from stores.
    pub fn new(accounts: A, storage: S, code: C) -> Self {
        Self { inner: Arc::new(RwLock::new(QmdbStore::new(accounts, storage, code))) }
    }

    /// Create from an existing `QmdbStore`.
    pub fn from_store(store: QmdbStore<A, S, C>) -> Self {
        Self { inner: Arc::new(RwLock::new(store)) }
    }

    /// Acquire read lock on the underlying store.
    pub fn read(&self) -> Result<RwLockReadGuard<'_, QmdbStore<A, S, C>>, HandleError> {
        self.inner.read().map_err(|_| HandleError::LockPoisoned)
    }

    /// Acquire write lock on the underlying store.
    pub fn write(&self) -> Result<RwLockWriteGuard<'_, QmdbStore<A, S, C>>, HandleError> {
        self.inner.write().map_err(|_| HandleError::LockPoisoned)
    }
}

impl<A, S, C> QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256> + QmdbBatchable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + QmdbBatchable<Key = B256, Value = Vec<u8>>,
{
    /// Commit changes atomically.
    pub fn commit(&self, changes: ChangeSet) -> Result<(), HandleError> {
        let mut store = self.write()?;
        store.commit_changes(changes)?;
        Ok(())
    }

    /// Initialize with genesis allocations.
    pub fn init_genesis(&self, allocs: Vec<(Address, U256)>) -> Result<(), HandleError> {
        use std::collections::BTreeMap;

        use alloy_primitives::KECCAK256_EMPTY;

        let mut changes = ChangeSet::new();
        for (address, balance) in allocs {
            changes.accounts.insert(
                address,
                AccountUpdate {
                    created: true,
                    selfdestructed: false,
                    nonce: 0,
                    balance,
                    code_hash: KECCAK256_EMPTY,
                    code: None,
                    storage: BTreeMap::new(),
                },
            );
        }
        self.commit(changes)
    }
}

impl<A, S, C> std::fmt::Debug for QmdbHandle<A, S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QmdbHandle").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap as StdHashMap, sync::Mutex};

    use kora_qmdb::{QmdbBatchable, QmdbGettable};

    use super::*;

    #[derive(Debug, Default)]
    struct MemoryStore<K, V> {
        data: Mutex<StdHashMap<K, V>>,
    }

    impl<K, V> MemoryStore<K, V> {
        fn new() -> Self {
            Self { data: Mutex::new(StdHashMap::new()) }
        }
    }

    #[derive(Debug)]
    struct MemoryError;

    impl std::fmt::Display for MemoryError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "memory error")
        }
    }

    impl std::error::Error for MemoryError {}

    impl<K: Clone + Eq + std::hash::Hash, V: Clone> QmdbGettable for MemoryStore<K, V> {
        type Error = MemoryError;
        type Key = K;
        type Value = V;

        fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }
    }

    impl<K: Clone + Eq + std::hash::Hash, V: Clone> QmdbBatchable for MemoryStore<K, V> {
        fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = (Self::Key, Option<Self::Value>)>,
        {
            let mut data = self.data.lock().unwrap();
            for (key, value) in ops {
                match value {
                    Some(v) => {
                        data.insert(key, v);
                    }
                    None => {
                        data.remove(&key);
                    }
                }
            }
            Ok(())
        }
    }

    type TestHandle = QmdbHandle<
        MemoryStore<Address, [u8; 80]>,
        MemoryStore<StorageKey, U256>,
        MemoryStore<B256, Vec<u8>>,
    >;

    fn create_test_handle() -> TestHandle {
        QmdbHandle::new(MemoryStore::new(), MemoryStore::new(), MemoryStore::new())
    }

    #[test]
    fn handle_is_clone() {
        let handle = create_test_handle();
        let _cloned = handle.clone();
    }

    #[test]
    fn init_genesis_creates_accounts() {
        let handle = create_test_handle();
        let allocs = vec![
            (Address::repeat_byte(0x01), U256::from(1000)),
            (Address::repeat_byte(0x02), U256::from(2000)),
        ];
        handle.init_genesis(allocs).unwrap();

        let store = handle.read().unwrap();
        let acc1 = store.get_account(&Address::repeat_byte(0x01)).unwrap().unwrap();
        assert_eq!(acc1.1, U256::from(1000));

        let acc2 = store.get_account(&Address::repeat_byte(0x02)).unwrap().unwrap();
        assert_eq!(acc2.1, U256::from(2000));
    }
}
