use crate::crypto::crypto_lib::evaluation_domain::smallest_power_of_2_greater_or_eq_than;
use reed_solomon_16::{ReedSolomonDecoder, ReedSolomonEncoder};
use serde::{Deserialize, Serialize};

type Bytes = Vec<u8>;

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct RSDecoderData {
    pub n: usize,
    pub t: usize,
    pub share_size: usize,
    pub pow_2_size: usize,
    pub original_message_len: usize,
}

pub struct RSDecoder {
    datas: RSDecoderData,
    decoder: ReedSolomonDecoder,
}

impl RSDecoder {
    pub fn new(datas: RSDecoderData) -> Self {
        Self {
            decoder: ReedSolomonDecoder::new(datas.t, datas.n, datas.pow_2_size).unwrap(),
            datas,
        }
    }

    pub fn datas(&self) -> RSDecoderData {
        self.datas
    }

    pub fn add_recovery_share(&mut self, i: usize, share: &Bytes) {
        self.decoder.add_recovery_shard(i, share).unwrap()
    }

    pub fn compute_all_shares(&mut self) -> Vec<Bytes> {
        let result = self.decoder.decode().unwrap();
        result
            .restored_original_iter()
            .map(|(_, b)| b.to_vec())
            .collect()
    }

    pub fn compute_secret_from_shares(&mut self, shares: Vec<Bytes>) -> Bytes {
        let mut message = Vec::with_capacity((self.datas.t) * self.datas.share_size);
        for mut share in shares {
            share.truncate(self.datas.share_size);
            message.append(&mut share);
        }
        message.truncate(self.datas.original_message_len);
        message
    }

    pub fn decode(&mut self) -> Bytes {
        let shares = self.compute_all_shares();
        self.compute_secret_from_shares(shares)
    }
}

pub fn reed_solomon_encode(mut message: Bytes, n: usize, t: usize) -> (Vec<Bytes>, RSDecoderData) {
    let original_message_len = message.len();
    while message.len() % t != 0 {
        message.push(0)
    }
    let share_size = message.len() / t;
    let mut pow_2_size = smallest_power_of_2_greater_or_eq_than(share_size).0;
    if pow_2_size < 64 {
        pow_2_size = 64
    }
    let mut encoder = ReedSolomonEncoder::new(t, n, pow_2_size).unwrap();

    for mut i in 0..t {
        i *= share_size;
        let mut msg = message[i..i + share_size].to_vec();
        msg.append(&mut vec![0; pow_2_size - share_size]);
        encoder.add_original_shard(&msg).unwrap()
    }
    let shares = encoder
        .encode()
        .unwrap()
        .recovery_iter()
        .map(|s| s.to_vec())
        .collect::<Vec<Bytes>>();
    let datas = RSDecoderData {
        original_message_len,
        n,
        t,
        share_size,
        pow_2_size,
    };
    (shares, datas)
}
