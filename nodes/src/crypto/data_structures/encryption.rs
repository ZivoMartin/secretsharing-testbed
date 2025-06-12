use crate::crypto::crypto_lib::vss::ni_vss::{
    encryption::CiphertextChunks, nizk_chunking::ProofChunking, nizk_sharing::ProofSharing,
};
use blstrs::G1Projective;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Encryption {
    pub ciphertext: CiphertextChunks,
    pub r_bb: G1Projective,
    pub enc_rr: Vec<G1Projective>,
    pub chunk_pf: ProofChunking,
    pub share_pf: ProofSharing,
}
