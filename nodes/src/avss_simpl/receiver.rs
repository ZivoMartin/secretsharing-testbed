use crate::{
    crypto::{
        crypto_lib::vss::{common::low_deg_test, ni_vss::encryption::dec_chunks},
        data_structures::{encryption::Encryption, keypair::PublicKey},
        gen_root, verify_encryption, Commitment, Share,
    },
    log,
    node::{configuration::Configuration, node::Node},
};
use aptos_crypto::bls12381::Signature;
use blstrs::G1Projective;
use global_lib::{
    enc, init_message,
    messages::{AvssSimplCommand, NameSpace},
    Wrapped,
};
use std::io::Write;

use super::crypto_messages::BroadcastReceiv;

pub async fn first_receiv(node: Wrapped<Node>, comm: Commitment, share: Share) {
    log!(node, "Avss Simpl: First receiv received");
    let mut node = node.lock().await;
    node.log("First Receiv begin");
    if low_deg_test(&comm, node.config()) && comm.verify(&share) {
        node.set_comm(comm);
        let sign = if node.is_byz() {
            node.fake_sign()
        } else {
            node.sign()
        };
        let mut buf = init_message(NameSpace::AvssSimpl, AvssSimplCommand::Ack);
        enc!((node.index(), sign), buf);
        node.save_share(share).await;
        node.contact_dealer(buf).await;
    } else {
        node.log("I received invalid share.");
    }
    node.log("First Receiv ended");
}

pub fn try_to_verify_encryption(
    missing: &[usize],
    config: &Configuration,
    encs: &[Encryption],
    comm: &Commitment,
    ekeys: &[PublicKey],
    missing_coms: &[Vec<G1Projective>],
) -> bool {
    !config.is_dual_threshold()
        || verify_encryption(config, encs, missing, missing_coms, comm, ekeys)
}

pub async fn verify_and_output(node: Wrapped<Node>, data: BroadcastReceiv) {
    {
        log!(node, "Avss Simpl: Verify and output received");
        let BroadcastReceiv {
            comm,
            signs,
            shares,
            missing_coms,
            encs,
            missing,
        } = data;
        let mut node = node.lock().await;
        let mut shares_set: Vec<bool> = vec![false; node.n() as usize];
        assert_eq!(signs.len() as u16, node.t() * 2 + 1);
        let root = gen_root(&enc!(comm));
        let mpk = signs
            .iter()
            .map(|(i, _)| node.get_specific_key(*i))
            .collect::<Vec<&PublicKey>>();

        let aggpk = PublicKey::aggregate(&mpk);
        let sig = Signature::aggregate(
            signs
                .into_iter()
                .map(|(i, s)| {
                    shares_set[i as usize] = true;
                    s
                })
                .collect(),
        )
        .unwrap();

        assert!(sig
            .verify_aggregate_arbitrary_msg(&[&root], &[&aggpk])
            .is_ok());

        assert!(comm.batch_verify(&shares));
        assert!(try_to_verify_encryption(
            &missing,
            node.config(),
            &encs,
            &comm,
            &node.get_all_pkey(),
            &missing_coms,
        ));
        missing.iter().for_each(|i| shares_set[*i] = true);
        if !node.has_share() && missing.contains(&node.uindex()) {
            let i: usize = missing.iter().position(|i| node.uindex() == *i).unwrap();
            let shares: Vec<blstrs::Scalar> = encs
                .iter()
                .map(|e| dec_chunks(&e.ciphertext, *node.my_decrypt_skey(), i).unwrap())
                .collect();
            let index = node.index();
            let l = shares.len();
            node.save_share(Share::new(index, shares, vec![blstrs::Scalar::from(0); l]))
                .await;
        }
        for s in shares {
            let i = s.uindex();
            assert!(!shares_set[i]);
            shares_set[i] = true;
            node.save_share(s).await;
        }
        assert!(!shares_set.contains(&false));
        node.set_comm(comm);
        node.log("Verify and output ended");
    }
    Node::output(node)
}
