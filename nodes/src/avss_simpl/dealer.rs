use blstrs::G1Projective;
use notifier_hub::notifier::MessageReceiver as Receiver;
use std::collections::HashMap;
use std::sync::Arc;

use super::crypto_messages::BroadcastReceiv;
use crate::{
    broadcast::broadcast_message_types::BroadcastMessageType,
    crypto::{
        compute_comm_and_shares, encode_shares, gen_root, is_valid_sign, Commitment, Share, Sign,
    },
    node::{node::Node, node_message::NodeMessage},
    panic_if_over,
};
use global_lib::{enc, messages::AvssSimplCommand, Wrapped};

type ShareMap = HashMap<u16, Share>;

async fn send_first_message(node: &Wrapped<Node>) -> (ShareMap, Commitment) {
    let mut node = node.lock().await;
    let (comm, mut output, secrets) = compute_comm_and_shares(node.config());
    node.set_secrets(secrets);
    let config = node.config();
    let mut shares = HashMap::<u16, Share>::new();
    let comm_parsed = enc!(comm);
    let cor = config.n() - config.dealer_corruption();
    let empty_share = Share::empty(config.batch_size() as usize);
    let messages = (0..config.n())
        .rev()
        .map(|i| {
            let share = if i >= cor {
                let mut share = empty_share.clone();
                share.set_index(i);
                share
            } else {
                output.pop().unwrap()
            };
            let mut buf = enc!(AvssSimpl, AvssSimplCommand::Share, share);
            buf.extend(&comm_parsed);
            shares.insert(
                i,
                if i >= cor {
                    output.pop().unwrap()
                } else {
                    share
                },
            );
            (i as usize, buf)
        })
        .collect::<Vec<(usize, Vec<u8>)>>();
    for (i, msg) in messages {
        node.contact(i, Arc::new(msg));
    }
    (shares, comm)
}

async fn wait_for_signs(
    node: &Wrapped<Node>,
    mut receiver: Receiver<NodeMessage>,
    shares: &mut ShareMap,
    root: &[u8],
) -> Vec<(u16, Sign)> {
    let threshold = 2 * node.lock().await.t() + 1;
    let mut signatures = Vec::<(u16, Sign)>::new();
    loop {
        let msg = panic_if_over!(receiver);
        match msg {
            NodeMessage::AvssSimplDealerMessage(i, sign)
                if is_valid_sign(node.lock().await.get_specific_key(i), &sign, root) =>
            {
                shares.remove(&i);
                signatures.push((i, sign));
                if signatures.len() == threshold as usize {
                    break;
                }
            }
            _ => (),
        }
    }
    signatures
}

async fn broadcast_missing_share(
    node: Wrapped<Node>,
    shares: ShareMap,
    comm: Commitment,
    signs: Vec<(u16, Sign)>,
) {
    let mut node = node.lock().await;
    let config = node.config();
    let mut missing_shares: Vec<Share> = shares.values().cloned().collect();
    missing_shares.sort();
    let (shares, encoded, missing) = if config.is_dual_threshold() {
        let r = config.r() as usize;
        (
            missing_shares[..r].to_vec(),
            missing_shares[r..].to_vec(),
            missing_shares[r..].iter().map(|s| s.uindex()).collect(),
        )
    } else {
        (missing_shares, Vec::<Share>::new(), Vec::new())
    };
    let missing_coms: Vec<Vec<G1Projective>> = encoded
        .iter()
        .map(|s| comm.get_col(s.index() as usize))
        .collect();
    let encs = encode_shares(node.config(), &comm, &node.get_all_pkey(), &encoded).await;
    let msg = BroadcastReceiv {
        comm,
        signs,
        missing_coms,
        shares,
        encs,
        missing,
    };
    node.reliable_broadcast(BroadcastMessageType::AvssSimpl, enc!(msg))
        .await;
}

pub async fn deal(node: Wrapped<Node>, receiver: Receiver<NodeMessage>) {
    node.lock().await.reset_timer();
    node.lock().await.log("AS DEALER: Sending first message");
    let (mut shares, comm) = send_first_message(&node).await;
    node.lock().await.log("AS DEALER: Waiting for signatures");
    let root = gen_root(&enc!(comm));
    let signatures = wait_for_signs(&node, receiver, &mut shares, &root).await;
    node.lock().await.log("AS DEALER: Broadcast missing shares");
    broadcast_missing_share(node, shares, comm, signatures).await;
}
