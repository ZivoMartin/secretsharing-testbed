use std::ops::Mul;

use crate::crypto::crypto_lib::vss::ni_vss::{
    chunking::{CHUNK_MAX, CHUNK_MIN, CHUNK_SIZE},
    utils::short_hash_for_linear_search,
};

use blstrs::{G1Projective, Scalar};
use group::Group;

pub struct HonestDealerDlogLookupTable {
    table: Vec<u32>,
}

lazy_static::lazy_static! {
    static ref LINEAR_DLOG_SEARCH: HonestDealerDlogLookupTable = HonestDealerDlogLookupTable::create();
}

impl HonestDealerDlogLookupTable {
    fn create() -> Self {
        let mut x = G1Projective::identity();

        let mut table = vec![0u32; CHUNK_SIZE];
        for i in CHUNK_MIN..=CHUNK_MAX {
            table[i as usize] = short_hash_for_linear_search(&x);
            x += G1Projective::generator();
        }

        Self { table }
    }

    pub fn new() -> &'static Self {
        &LINEAR_DLOG_SEARCH
    }

    /// Solve several discrete logarithms
    pub fn solve_several(&self, targets: &[G1Projective]) -> Vec<Option<Scalar>> {
        use subtle::{ConditionallySelectable, ConstantTimeEq};

        let target_hashes = targets
            .iter()
            .map(short_hash_for_linear_search)
            .collect::<Vec<_>>();

        // This code assumes that CHUNK_MAX fits in a u16
        let mut scan_results = vec![0u16; targets.len()];

        for x in CHUNK_MIN..=CHUNK_MAX {
            let x_hash = self.table[x as usize];

            for i in 0..targets.len() {
                let hashes_eq = x_hash.ct_eq(&target_hashes[i]);
                scan_results[i].conditional_assign(&(x as u16), hashes_eq);
            }
        }

        // Now confirm the results (since collisions may have occurred
        // if the dealer was dishonest) and convert to Scalar

        let mut results = Vec::with_capacity(targets.len());

        for i in 0..targets.len() {
            /*
            After finding a candidate we must perform a multiplication in order
            to tell if we found the dlog correctly, or if there was a collision
            due to a dishonest dealer.

            If no match was found then scan_results[i] will just be zero, we
            perform the multiplication anyway and then reject the candidate dlog.
             */
            if G1Projective::generator().mul(Scalar::from(scan_results[i] as u64)) == targets[i] {
                results.push(Some(Scalar::from(scan_results[i] as u64)));
            } else {
                results.push(None);
            }
        }

        results
    }
}
