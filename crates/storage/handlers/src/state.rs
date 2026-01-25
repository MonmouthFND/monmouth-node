//! StateDb trait implementations for QmdbHandle.

use alloy_primitives::{Address, B256, Bytes, KECCAK256_EMPTY, U256};
use kora_qmdb::{AccountEncoding, ChangeSet, QmdbBatchable, QmdbGettable, StateRoot, StorageKey};
use kora_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};

use crate::QmdbHandle;

impl<A, S, C> StateDbRead for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]> + Send + Sync + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256> + Send + Sync + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + Send + Sync + 'static,
{
    fn nonce(&self, address: &Address) -> Result<u64, StateDbError> {
        let store = self.read().map_err(|_| StateDbError::LockPoisoned)?;
        match store.get_account(address).map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((nonce, _, _, _)) => Ok(nonce),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    fn balance(&self, address: &Address) -> Result<U256, StateDbError> {
        let store = self.read().map_err(|_| StateDbError::LockPoisoned)?;
        match store.get_account(address).map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((_, balance, _, _)) => Ok(balance),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    fn code_hash(&self, address: &Address) -> Result<B256, StateDbError> {
        let store = self.read().map_err(|_| StateDbError::LockPoisoned)?;
        match store.get_account(address).map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((_, _, code_hash, _)) => Ok(code_hash),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    fn code(&self, code_hash: &B256) -> Result<Bytes, StateDbError> {
        if *code_hash == KECCAK256_EMPTY || *code_hash == B256::ZERO {
            return Ok(Bytes::new());
        }
        let store = self.read().map_err(|_| StateDbError::LockPoisoned)?;
        store.get_code(code_hash).map_err(|e| StateDbError::Storage(e.to_string()))?.map_or_else(
            || Err(StateDbError::CodeNotFound(*code_hash)),
            |bytes| Ok(Bytes::from(bytes)),
        )
    }

    fn storage(&self, address: &Address, slot: &U256) -> Result<U256, StateDbError> {
        let store = self.read().map_err(|_| StateDbError::LockPoisoned)?;

        // Get account to find generation
        let generation =
            match store.get_account(address).map_err(|e| StateDbError::Storage(e.to_string()))? {
                Some((_, _, _, generation)) => generation,
                None => return Ok(U256::ZERO),
            };

        let key = StorageKey::new(*address, generation, *slot);
        Ok(store
            .get_storage(&key)
            .map_err(|e| StateDbError::Storage(e.to_string()))?
            .unwrap_or(U256::ZERO))
    }
}

impl<A, S, C> StateDbWrite for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + Send
        + Sync
        + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256>
        + QmdbBatchable<Key = StorageKey, Value = U256>
        + Send
        + Sync
        + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>
        + QmdbBatchable<Key = B256, Value = Vec<u8>>
        + Send
        + Sync
        + 'static,
{
    fn commit(&self, changes: ChangeSet) -> Result<B256, StateDbError> {
        let mut store = self.write().map_err(|_| StateDbError::LockPoisoned)?;
        store.commit_changes(changes).map_err(|e| StateDbError::Storage(e.to_string()))?;

        // Return placeholder root for now
        // TODO: Implement proper state root computation
        Ok(B256::ZERO)
    }

    fn compute_root(&self, _changes: &ChangeSet) -> Result<B256, StateDbError> {
        // TODO: Implement speculative root computation
        Ok(StateRoot::compute(B256::ZERO, B256::ZERO, B256::ZERO))
    }

    fn merge_changes(&self, mut older: ChangeSet, newer: ChangeSet) -> ChangeSet {
        older.merge(newer);
        older
    }
}

impl<A, S, C> StateDb for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + Send
        + Sync
        + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256>
        + QmdbBatchable<Key = StorageKey, Value = U256>
        + Send
        + Sync
        + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>
        + QmdbBatchable<Key = B256, Value = Vec<u8>>
        + Send
        + Sync
        + 'static,
{
    fn state_root(&self) -> Result<B256, StateDbError> {
        // TODO: Implement proper state root retrieval
        Ok(B256::ZERO)
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
    fn state_db_returns_error_for_missing_account() {
        let handle = create_test_handle();
        let result = handle.nonce(&Address::ZERO);
        assert!(matches!(result, Err(StateDbError::AccountNotFound(_))));
    }

    #[test]
    fn state_db_returns_zero_for_missing_storage() {
        let handle = create_test_handle();
        let result = handle.storage(&Address::ZERO, &U256::from(1)).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn state_db_returns_empty_for_empty_code() {
        let handle = create_test_handle();
        let result = handle.code(&KECCAK256_EMPTY).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn state_db_merge_changes() {
        let handle = create_test_handle();
        let older = ChangeSet::new();
        let newer = ChangeSet::new();
        let merged = handle.merge_changes(older, newer);
        assert!(merged.is_empty());
    }
}
