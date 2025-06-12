pub mod crypto_lib;
pub mod crypto_set;
pub mod data_structures;
pub mod scheme;

pub use data_structures::{
    bivariate_polynomial::{BiVariatePoly, Polynomial},
    commitment::Commitment,
    share::Share,
    Secret, Sign,
};
pub use scheme::*;

use sha2::{Digest, Sha256};
pub fn gen_root(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
