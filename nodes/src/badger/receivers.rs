use super::broadcast_recv::BroadcastReceiv;
use crate::{
    crypto::crypto_lib::vss::{
        common::low_deg_test,
        ni_vss::{dealing::verify_dealing, encryption::dec_chunks},
    },
    log,
    node::node::Node,
};

use global_lib::Wrapped;
use std::{io::Write, sync::Arc};

pub async fn decrypt_shares(node: Wrapped<Node>, data: BroadcastReceiv) {
    log!(node, "Decrypting shares..");
    let BroadcastReceiv { comm, encs } = data;
    {
        let mut node = node.lock().await;
        node.log("Preparing to decrypt shares..");
        let keys = Arc::new(
            node.get_all_pkey()
                .iter()
                .map(|k| k.c_key())
                .collect::<Vec<_>>(),
        );
        node.set_comm(comm.clone());

        let encs = Arc::new(encs);
        if !low_deg_test(&comm, node.config()) {
            panic!("Low deg test failed")
        }

        node.log("Low deg test passed");
        let base = comm.base()[1];
        let index: usize = node.uindex();
        let skey = *node.my_decrypt_skey();
        let _shares = (0..node.config().batch_size())
            .map(|i| {
                let keys = keys.clone();
                let encs = encs.clone();
                let comm = comm.clone();
                tokio::spawn(async move {
                    assert!(verify_dealing(&base, &comm.all()[i], &keys, &encs[i]));
                    (dec_chunks(&encs[i].ciphertext, skey, index), i)
                })
            })
            .collect::<Vec<_>>();
        node.log("Dec chunck passed");
    }
    log!(node, "Outputing");
    Node::output(node);
}
