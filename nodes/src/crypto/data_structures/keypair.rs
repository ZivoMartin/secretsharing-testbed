use crate::crypto::{crypto_lib::random_scalar, gen_root};
pub use aptos_crypto::bls12381::{PrivateKey as SigningPrivateKey, PublicKey as SigningPublicKey};
use aptos_crypto::{test_utils::KeyPair as SigningKeyPair, SigningKey, Uniform};
use blstrs::{G1Projective, Scalar};
use blsttc::{Ciphertext, SecretKey};
use serde::{Deserialize, Serialize};
use std::ops::Mul;

pub type PubEncryptionKey = G1Projective;
pub type PrivEncryptionKey = Scalar;

use super::{Base, Sign};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct PublicKey {
    signing_pkey: SigningPublicKey,
    crypting_key: PubEncryptionKey,
    blsttr_k: blsttc::PublicKey,
}

impl PublicKey {
    pub fn new(
        signing_pkey: SigningPublicKey,
        crypting_key: G1Projective,
        blsttr_k: blsttc::PublicKey,
    ) -> Self {
        Self {
            signing_pkey,
            crypting_key,
            blsttr_k,
        }
    }

    pub fn c_key(&self) -> PubEncryptionKey {
        self.crypting_key
    }

    pub fn s_key(&self) -> &SigningPublicKey {
        &self.signing_pkey
    }

    pub fn aggregate(keys: &[&Self]) -> SigningPublicKey {
        SigningPublicKey::aggregate(keys.iter().map(|k| &k.signing_pkey).collect()).unwrap()
    }

    pub fn blstt_encrypt(&self, msg: Vec<u8>) -> Ciphertext {
        self.blsttr_k.encrypt(msg)
    }
}

#[derive(Clone)]
pub struct KeyPair {
    signing_keypair: SigningKeyPair<SigningPrivateKey, SigningPublicKey>,
    crypting_keypair: (PrivEncryptionKey, PubEncryptionKey),
    blsttr_k: SecretKey,
}

impl KeyPair {
    pub fn generate<R>(base: &Base, rng: &mut R) -> Self
    where
        R: rand_core::RngCore + rand::CryptoRng,
    {
        let s_key = random_scalar(rng);
        let p_key = base[0].mul(s_key);
        Self {
            crypting_keypair: (s_key, p_key),
            signing_keypair: SigningKeyPair::generate(rng),
            blsttr_k: blsttc::SecretKey::random(),
        }
    }

    pub fn fake_sign(&self, bytes: &[u8]) -> Sign {
        let mut root: [u8; 32] = gen_root(bytes);
        root[0] = root[1];
        self.signing_keypair
            .private_key
            .sign_arbitrary_message(root.as_slice())
    }

    pub fn sign(&self, bytes: &[u8]) -> Sign {
        let root = gen_root(bytes);
        self.signing_keypair
            .private_key
            .sign_arbitrary_message(root.as_slice())
    }

    pub fn private_decrypt_key(&self) -> &PrivEncryptionKey {
        &self.crypting_keypair.0
    }

    pub fn private_blstt_decrypt_key(&self) -> &SecretKey {
        &self.blsttr_k
    }

    pub fn public_signing_key(&self) -> &SigningPublicKey {
        &self.signing_keypair.public_key
    }

    pub fn extract_public_key(&self) -> PublicKey {
        PublicKey::new(
            self.signing_keypair.public_key.clone(),
            self.crypting_keypair.1,
            self.blsttr_k.public_key(),
        )
    }

    pub fn blstt_decrypt(&self, msg: Ciphertext) -> Vec<u8> {
        self.blsttr_k.decrypt(&msg).unwrap()
    }
}
