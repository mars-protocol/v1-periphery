use ethereum_types::{H256};
use tiny_keccak::{Hasher, Keccak};
use std::str;
use sha3::{Digest, Keccak256};
use k256::ecdsa::VerifyingKey;
use k256::EncodedPoint;


/// Derives the Ethereum public key (lower-case without the `0x` prefix) from the ECDSA/secp256k1 verification key
pub fn get_public_key_from_verify_key(verify_key: &VerifyingKey, ) -> String {
    let mut hasher = Keccak256::new();
    let point: EncodedPoint = EncodedPoint::from(verify_key);
    let point = point.decompress();
    let point = point.unwrap();
    hasher.update(&point.as_bytes()[1..]);
    let verify_key = &hasher.finalize()[12..];
    let verify_key_str = hex::encode(verify_key.clone());
    return verify_key_str;
}


/// Normalizes recovery id for recoverable signature.
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-core/src/types/signature.rs#L142
pub fn normalize_recovery_id(v: u8) -> u8 {
    match v {
        0 => 0,
        1 => 1,
        27 => 0,
        28 => 1,
        v if v >= 35 => ((v - 1) % 2) as _,
        _ => 4,
    }
}

/// Hash a message according to EIP-191.
/// The data is a UTF-8 encoded string and will enveloped as follows:
/// `"\x19Ethereum Signed Message:\n" + message.length + message` and hashed using keccak256.
pub fn hash_message<S>(message: S) -> H256 where S: AsRef<[u8]>, {
    let message = message.as_ref();
    const PREFIX: &str = "\x19Ethereum Signed Message:\n";
    let mut eth_message = format!("{}{}", PREFIX, message.len()).into_bytes();
    eth_message.extend_from_slice(message);

    keccak256(&eth_message).into()
}

/// Compute the Keccak-256 hash of input bytes.
pub fn keccak256<S>(bytes: S) -> [u8; 32] where S: AsRef<[u8]>,{
    let mut output = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes.as_ref());
    hasher.finalize(&mut output);
    output
}

