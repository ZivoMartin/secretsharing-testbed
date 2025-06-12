//! Dealing phase of Groth20-BLS12-381 non-interactive verifier secret sharing
use std::ops::{Add, Mul};

use crate::crypto::{crypto_lib::random_scalar, data_structures::encryption::Encryption};
use blstrs::{G1Projective, Scalar};
use group::Group;
use rand::thread_rng;

use super::{
    encryption::{encrypt_and_prove, verify_chunk_proofs},
    nizk_sharing::{prove_sharing, verify_sharing, SharingWitness},
};

/// Generate the NiVSS transcript
pub fn create_dealing(
    h: &G1Projective,
    commits: &[G1Projective],
    receiver_keys: &[G1Projective],
    shares: &[Scalar],
    randomness: &[Scalar],
) -> Encryption {
    let (ciphertext, chunk_pf, r_a) = encrypt_and_prove(receiver_keys, shares);

    let g1 = G1Projective::generator();

    // Computing ElGamal ciphertexts of h^r
    let mut rng = thread_rng();
    let r_b = random_scalar(&mut rng);
    let r_bb = g1.mul(r_b);
    let enc_rr = randomness
        .iter()
        .zip(receiver_keys.iter())
        .map(|(r, pk)| h.mul(r).add(pk.mul(&r_b)))
        .collect::<Vec<G1Projective>>();

    let enc_ss = shares
        .iter()
        .zip(receiver_keys.iter())
        .map(|(s, pk)| g1.mul(s).add(pk.mul(&r_a)))
        .collect::<Vec<G1Projective>>();

    let r_aa = g1.mul(&r_a);
    let witness = SharingWitness::new(r_a, r_b, shares.to_vec(), randomness.to_vec());

    let share_pf = prove_sharing(
        h,
        commits,
        receiver_keys,
        &r_aa,
        &enc_ss,
        &r_bb,
        &enc_rr,
        &witness,
    );

    Encryption {
        ciphertext,
        r_bb,
        enc_rr,
        chunk_pf,
        share_pf,
    }
}

/// Verify the NiVSS transcript
pub fn verify_dealing(
    h: &G1Projective,
    commits: &[G1Projective],
    public_keys: &[G1Projective],
    Encryption {
        ciphertext,
        r_bb,
        enc_rr,
        chunk_pf,
        share_pf,
        ..
    }: &Encryption,
) -> bool {
    let valid_share = verify_sharing(h, commits, ciphertext, public_keys, r_bb, enc_rr, share_pf);
    valid_share && verify_chunk_proofs(public_keys, ciphertext, chunk_pf)
}
