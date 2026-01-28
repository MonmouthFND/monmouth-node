use alloy_consensus::{SignableTransaction as _, TxEip1559, TxEnvelope};
use alloy_primitives::{Address, Bytes, Signature, TxKind, U256, keccak256};
use k256::ecdsa::SigningKey;
use kora_domain::Tx;
use sha3::{Digest as _, Keccak256};

pub(crate) const CHAIN_ID: u64 = 1337;

const SENDER_KEY_BYTES: [u8; 32] = [1u8; 32];
const RECEIVER_KEY_BYTES: [u8; 32] = [2u8; 32];

pub(crate) fn sender_key() -> SigningKey {
    SigningKey::from_bytes(&SENDER_KEY_BYTES.into()).expect("valid sender key")
}

pub(crate) fn receiver_key() -> SigningKey {
    SigningKey::from_bytes(&RECEIVER_KEY_BYTES.into()).expect("valid receiver key")
}

pub(crate) fn address_from_key(key: &SigningKey) -> Address {
    let encoded = key.verifying_key().to_encoded_point(false);
    let pubkey = encoded.as_bytes();
    let hash = keccak256(&pubkey[1..]);
    Address::from_slice(&hash[12..])
}

pub(crate) fn sign_eip1559_transfer(
    key: &SigningKey,
    to: Address,
    value: U256,
    nonce: u64,
    gas_limit: u64,
) -> Tx {
    let tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce,
        gas_limit,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(to),
        value,
        access_list: Default::default(),
        input: Bytes::new(),
    };

    let digest = Keccak256::new_with_prefix(tx.encoded_for_signing());
    let (sig, recid) = key.sign_digest_recoverable(digest).expect("sign tx");
    let signature = Signature::from((sig, recid));
    let signed = tx.into_signed(signature);
    let envelope = TxEnvelope::from(signed);
    Tx::new(Bytes::from(alloy_rlp::encode(envelope)))
}
