//! Hashing to group elements (fields, curves)
use blstrs::{G1Projective, Scalar};
use more_asserts::{debug_assert_ge, debug_assert_le};

use crate::{
    crypto::crypto_lib::{
        hash_to_scalar, util, vss::public_parameters::PublicParameters, SCALAR_NUM_BYTES,
    },
    node::configuration::Configuration,
};

use super::{encryption::CiphertextChunks, nizk_chunking::SECURITY_LEVEL};

pub const NIVSS_DOM_SEP: &[u8; 13] = b"NIVSS_DOM_SEP";
pub const NIVSS_FIRST_MOVE: &[u8; 16] = b"NIVSS_FIRST_MOVE";
pub const NIVSS_HASH_TO_SCALAR_DST: &[u8; 24] = b"NIVSS_HASH_TO_SCALAR_DST";

#[allow(dead_code)]
pub trait FiatShamirProtocol {
    /// Append a domain separator for the PVSS protocol, consisting of a sharing configuration `sc`,
    /// which locks in the $t$ out of $n$ threshold.
    fn nivss_domain_sep(&mut self, sc: &Configuration);

    /// Append the public parameters `pp`
    fn append_public_parameters(&mut self, pp: &PublicParameters);

    /// Append the encryption keys `eks`.
    fn append_encryption_keys(&mut self, eks: &[G1Projective]);

    /// Appends one or more points to the transcript
    fn append_chunks_ciphertext(&mut self, ctxt: &CiphertextChunks);

    /// Returns the chunks for first message
    fn challenge_first_move(self, n: usize) -> Vec<Scalar>;

    fn challenge_scalar(&mut self) -> Scalar;
}

#[allow(non_snake_case)]
impl FiatShamirProtocol for merlin::Transcript {
    fn nivss_domain_sep(&mut self, sc: &Configuration) {
        self.append_message(b"dom-sep", NIVSS_DOM_SEP);
        self.append_u64(b"t", sc.get_threshold() as u64);
        self.append_u64(b"n", sc.n() as u64);
    }

    fn append_public_parameters(&mut self, pp: &PublicParameters) {
        self.append_message(b"pp", pp.to_bytes().as_slice());
    }

    fn append_encryption_keys(&mut self, eks: &[G1Projective]) {
        util::append_g1_vector(self, b"encryption-keys", eks);
    }

    fn append_chunks_ciphertext(&mut self, ctxts: &CiphertextChunks) {
        util::append_g1_vector(self, b"", &ctxts.rr.to_vec());
        ctxts
            .cc
            .iter()
            .for_each(|ctxt| util::append_g1_vector(self, b"", ctxt));
    }

    fn challenge_first_move(mut self, n: usize) -> Vec<Scalar> {
        challenge_random_linear_combination_scalars(&mut self, NIVSS_FIRST_MOVE, n)
    }

    fn challenge_scalar(&mut self) -> Scalar {
        let mut buf = [0u8; 64];
        self.challenge_bytes(b"challenge_c", &mut buf);

        hash_to_scalar(buf.as_slice(), NIVSS_HASH_TO_SCALAR_DST)
    }
}

// TODO(Performance): If randomly picking each coefficient is too slow, consider doing 1, r, r^2, r^3, etc
// FIXME: I am copying this function from `sacrificing_pvss::fiat_shamir`.
fn challenge_random_linear_combination_scalars(
    t: &mut merlin::Transcript,
    label: &'static [u8],
    n: usize,
) -> Vec<Scalar> {
    let num_bytes = SECURITY_LEVEL / 8;

    // TODO(Security): Audit! Make sure all exponents are indeed {SECURITY_PARAMETER}-bit scalars.
    // let mut two_to_128 = Scalar::one() + Scalar::one(); // 2
    // for _ in 0..7 {
    //     two_to_128.square_assign(); // 2^2, 2^4, 2^8, ..., 2^128
    // }

    debug_assert_le!(num_bytes, SCALAR_NUM_BYTES); // lambda cannot be bigger than a scalar
    debug_assert_ge!(num_bytes, 16); // lambda >= 128

    let buf_size = n * num_bytes;
    let mut buf = vec![0; buf_size];
    buf.resize(buf_size, 0u8);
    t.challenge_bytes(label, &mut buf);

    let scalars = buf
        .chunks_exact(num_bytes)
        .map(|chunk| {
            let mut slice = [0u8; SCALAR_NUM_BYTES];
            slice[..chunk.len()].copy_from_slice(chunk);
            Scalar::from_bytes_le(&slice).unwrap()
        })
        .collect::<Vec<Scalar>>();

    debug_assert_eq!(scalars.len(), n);

    scalars
}
