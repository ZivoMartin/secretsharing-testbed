use global_lib::{enc, messages::DispRetCommand};
use sha2::{Digest, Sha256};

use crate::crypto::data_structures::reed_solomon_code::reed_solomon_encode;

use super::messages::Propose;

// Supports batching
pub fn get_messages(messages: Vec<Vec<u8>>, n: usize, t: usize) -> Vec<Vec<u8>> {
    let mut decoders = Vec::with_capacity(messages.len());
    let mut codes = messages
        .into_iter()
        .map(|m| {
            let (codes, decoder) = reed_solomon_encode(m, n, t);
            decoders.push(decoder);
            codes
        })
        .collect::<Vec<_>>();
    let rev_codes = (0..n)
        .map(|_| {
            codes
                .iter_mut()
                .map(|code| code.pop().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut hasher = Sha256::new();
    let hashes = rev_codes
        .iter()
        .map(|codes| {
            codes
                .iter()
                .map(|code| {
                    hasher.update(code);
                    hasher.finalize_reset().to_vec()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    rev_codes
        .into_iter()
        .map(|codes| {
            let propose = Propose::new(hashes.clone(), codes, decoders.clone());
            enc!(DisperseRetrieve, DispRetCommand::Propose, propose)
        })
        .collect()
}
