//! REVM database trait implementations.

use alloy_primitives::{Address, B256, Bytes, KECCAK256_EMPTY, U256};
use kora_qmdb::{AccountEncoding, QmdbBatchable, QmdbGettable, StorageKey};
use revm::{
    bytecode::Bytecode,
    database_interface::{DatabaseCommit, DatabaseRef},
    primitives::HashMap,
    state::Account,
};

use crate::{error::HandleError, qmdb::QmdbHandle};

impl<A, S, C> DatabaseRef for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>,
{
    type Error = HandleError;

    fn basic_ref(&self, address: Address) -> Result<Option<revm::state::AccountInfo>, Self::Error> {
        let store = self.read()?;
        match store.get_account(&address)? {
            Some((nonce, balance, code_hash, _gen)) => {
                Ok(Some(revm::state::AccountInfo { nonce, balance, code_hash, code: None }))
            }
            None => Ok(None),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == KECCAK256_EMPTY || code_hash == B256::ZERO {
            return Ok(Bytecode::default());
        }
        let store = self.read()?;
        match store.get_code(&code_hash)? {
            Some(bytes) => Ok(Bytecode::new_raw(Bytes::from(bytes))),
            None => Err(HandleError::CodeNotFound(code_hash)),
        }
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let store = self.read()?;

        // Get account to find generation
        let generation = match store.get_account(&address)? {
            Some((_, _, _, gen)) => gen,
            None => return Ok(U256::ZERO),
        };

        let key = StorageKey::new(address, generation, index);
        match store.get_storage(&key)? {
            Some(value) => Ok(value),
            None => Ok(U256::ZERO),
        }
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        Err(HandleError::BlockHashNotFound(number))
    }
}

impl<A, S, C> DatabaseCommit for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256> + QmdbBatchable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + QmdbBatchable<Key = B256, Value = Vec<u8>>,
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        use std::collections::BTreeMap;

        use kora_qmdb::{AccountUpdate, ChangeSet};

        let mut changeset = ChangeSet::new();

        for (address, account) in changes {
            if !account.is_touched() {
                continue;
            }

            let storage: BTreeMap<U256, U256> =
                account.storage.iter().map(|(k, v)| (*k, v.present_value())).collect();

            let code = account.info.code.as_ref().map(|c| c.bytes().to_vec());

            changeset.accounts.insert(
                address,
                AccountUpdate {
                    created: account.is_created(),
                    selfdestructed: account.is_selfdestructed(),
                    nonce: account.info.nonce,
                    balance: account.info.balance,
                    code_hash: account.info.code_hash,
                    code,
                    storage,
                },
            );
        }

        // Ignore errors in DatabaseCommit (matches REVM's signature)
        let _ = QmdbHandle::commit(self, changeset);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap as StdHashMap, sync::Mutex};

    use kora_qmdb::{QmdbBatchable, QmdbGettable};
    use revm::database_interface::DatabaseRef;

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
    fn basic_ref_returns_none_for_missing() {
        let handle = create_test_handle();
        let result = handle.basic_ref(Address::ZERO).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn storage_ref_returns_zero_for_missing() {
        let handle = create_test_handle();
        let result = handle.storage_ref(Address::ZERO, U256::from(1)).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn code_by_hash_returns_empty_for_keccak_empty() {
        let handle = create_test_handle();
        let result = handle.code_by_hash_ref(KECCAK256_EMPTY).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn block_hash_returns_error() {
        let handle = create_test_handle();
        let result = handle.block_hash_ref(100);
        assert!(matches!(result, Err(HandleError::BlockHashNotFound(100))));
    }
}
