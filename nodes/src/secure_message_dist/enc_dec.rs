use blstrs::Scalar;
pub type Key = Scalar;
use sha2::{Digest, Sha512};

pub fn decrypt(bytes: &[u8], key: &Scalar) -> Vec<u8> {
    let key_bytes = key.to_bytes_be();
    bytes
        .iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ key_bytes[i % key_bytes.len()])
        .collect()
}

pub fn encrypt(bytes: &[u8], key: &Scalar) -> Vec<u8> {
    decrypt(bytes, key)
}

pub fn get_key(i: usize, e: &Scalar) -> Key {
    let mut hasher = Sha512::new();
    hasher.update(i.to_be_bytes());
    hasher.update(e.to_bytes_be());
    let bytes = &hasher.finalize().to_vec()[..8];
    Scalar::from(u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}
