use serde::{Deserialize, Serialize};

use crate::crypto::data_structures::reed_solomon_code::RSDecoderData;

pub type Share = Vec<u8>;
pub type Hash = Vec<u8>;
pub type HashesSet = Vec<Vec<Hash>>;

#[derive(Serialize, Deserialize, Clone)]
pub struct Propose {
    hash_vec: HashesSet,
    shares: Vec<Share>,
    decoder: Vec<RSDecoderData>,
}

impl Propose {
    pub fn new(
        hash_vec: Vec<Vec<Hash>>,
        shares: Vec<Share>, // one for each message
        decoder: Vec<RSDecoderData>,
    ) -> Self {
        Self {
            hash_vec,
            shares,
            decoder,
        }
    }

    pub fn extract(self) -> (HashesSet, Vec<Share>, Vec<RSDecoderData>) {
        let Propose {
            hash_vec,
            shares,
            decoder,
        } = self;
        (hash_vec, shares, decoder)
    }
}

pub type Ready = Echo;

#[derive(Serialize, Clone, Deserialize)]
pub struct Echo {
    index: usize,
    hash_vec: Vec<Vec<Hash>>,
    shares: Vec<Share>,
    decoder: Vec<RSDecoderData>,
}

impl Echo {
    pub fn new(
        index: usize,
        hash_vec: Vec<Vec<Hash>>,
        shares: Vec<Share>,
        decoder: Vec<RSDecoderData>,
    ) -> Self {
        Self {
            index,
            hash_vec,
            shares,
            decoder,
        }
    }

    pub fn t(&self) -> u16 {
        self.decoder[0].t as u16
    }

    pub fn extract(self) -> (usize, HashesSet, Vec<Share>, Vec<RSDecoderData>) {
        let Echo {
            index,
            hash_vec,
            shares,
            decoder,
        } = self;
        (index, hash_vec, shares, decoder)
    }
}
