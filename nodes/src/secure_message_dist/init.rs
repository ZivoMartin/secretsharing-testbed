use super::{
    enc_dec::{encrypt, get_key},
    messages::ProposeMessage,
};
use crate::crypto::data_structures::{
    bivariate_polynomial::Polynomial, merkle_tree::hash_leafs,
    reed_solomon_code::reed_solomon_encode,
};
use blstrs::Scalar;

use rand::thread_rng;

type Bytes = Vec<u8>;

pub fn get_secure_message_dis_transcripts(messages: Vec<Bytes>, n: usize, _: usize) -> Vec<Bytes> {
    assert!(messages.len() == n);
    let t = n / 3;
    let mut shares_and_proofs = vec![Vec::with_capacity(n); n];
    let mut datas = Vec::with_capacity(n);

    let rng = &mut thread_rng();

    let mut evals = vec![Vec::with_capacity(n); n];
    let mut keys = (0..n)
        .map(|_| {
            let p = Polynomial::random(None, n - 2 * t - 1, rng);
            (0..=n)
                .map(|j| {
                    let eval = p.eval(&Scalar::from(j as u64));
                    if j != 0 {
                        evals[j - 1].push(eval);
                    }
                    get_key(j, &eval)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let messages = messages
        .into_iter()
        .enumerate()
        .map(|(i, message)| {
            let key = keys[i].remove(0);
            reed_solomon_encode(encrypt(&message, &key), n, n - 2 * t)
        })
        .collect::<Vec<_>>();

    for ((shares, data), key_line) in messages.into_iter().zip(keys.into_iter()) {
        datas.push(data);
        let enc_shares = shares
            .iter()
            .zip(key_line.into_iter())
            .map(|(s, k)| {
                let mut s = s.clone();
                s.append(&mut k.to_bytes_be().to_vec());
                s
            })
            .collect::<Vec<_>>();
        let proofs = hash_leafs(enc_shares);
        for ((sp_vec, proof), share) in shares_and_proofs.iter_mut().zip(proofs).zip(shares) {
            sp_vec.push((proof, share))
        }
    }
    shares_and_proofs
        .into_iter()
        .zip(datas)
        .zip(evals)
        .map(|((shares_and_proofs, datas), eval_line)| {
            ProposeMessage::get_transcript(datas, shares_and_proofs, eval_line)
        })
        .collect()
}
