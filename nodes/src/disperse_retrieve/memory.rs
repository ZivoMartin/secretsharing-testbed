use std::collections::HashSet;

use crate::crypto::data_structures::reed_solomon_code::{reed_solomon_encode, RSDecoder};
use crate::{
    break_if_over,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::enc;
use global_lib::messages::DispRetCommand;
use global_lib::Wrapped;
use sha2::{Digest, Sha256};

use super::messages::{Echo, HashesSet, Ready, Share};

pub fn check(i: usize, shares: &[Share], set: &HashesSet) -> Result<(), ()> {
    let mut hasher = Sha256::new();

    for (share, hash) in shares.iter().zip(set[i].iter()) {
        hasher.update(share);
        if hasher.finalize_reset().to_vec() != *hash {
            return Err(());
        }
    }

    Ok(())
}

async fn interpolate_remaining_shares(
    node: &Wrapped<Node>,
    hash_set: HashesSet,
    index: usize,
    results: &mut Vec<Vec<u8>>,
    decoders: &mut Vec<RSDecoder>,
) {
    let mut rev_shares = decoders
        .iter_mut()
        .map(|d| {
            let sec = d.decode();
            results.push(sec.clone());
            reed_solomon_encode(sec, d.datas().n, d.datas().t).0
        })
        .collect::<Vec<_>>();

    let n = rev_shares.len();
    let shares = (0..n)
        .map(|_| {
            rev_shares
                .iter_mut()
                .map(|code| code.pop().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let my_share = shares[index].clone();

    let echo = enc!(
        DisperseRetrieve,
        DispRetCommand::Ready,
        Ready::new(
            index,
            hash_set,
            my_share,
            decoders.iter().map(|d| d.datas()).collect()
        )
    );
    node.lock().await.broadcast(echo, true).await;
}

pub async fn echo_manager(node: Wrapped<Node>) {
    let mut echo_count = 0;
    let mut receiver = node.lock().await.subscribe(NodeMessage::DispRetEchoConst);

    loop {
        let echo = break_if_over!(receiver);
        match echo {
            NodeMessage::DispRetEcho(echo) => {
                echo_count += 1;
                if echo_count <= echo.t() {
                    let output = echo_count == echo.t();

                    let _ = node
                        .lock()
                        .await
                        .send_message(NodeMessage::DispRetAddShare(echo))
                        .await
                        .wait(None)
                        .await;

                    if output {
                        node.lock()
                            .await
                            .send_message(NodeMessage::DispRetOutputReq)
                            .await;
                    }
                }
            }
            _ => panic!("Unexpected message"),
        }
    }
}

pub async fn ready_manager(node: Wrapped<Node>) {
    let mut ready_count = 0;
    let mut receiver = node.lock().await.subscribe(NodeMessage::DispRetEchoConst);

    loop {
        let echo = break_if_over!(receiver);
        match echo {
            NodeMessage::DispRetEcho(echo) => {
                ready_count += 1;
                let output = ready_count == echo.t();
                if ready_count <= echo.t() {
                    let _ = node
                        .lock()
                        .await
                        .send_message(NodeMessage::DispRetAddShare(echo))
                        .await
                        .wait(None)
                        .await;
                    if output {
                        node.lock()
                            .await
                            .send_message(NodeMessage::DispRetOutputReq)
                            .await;
                    }
                }
            }
            _ => panic!("Unexpected message"),
        }
    }
}

pub async fn messages_handler(node: Wrapped<Node>) {
    let mut receiver = node.lock().await.subscribe_multiple(&[
        NodeMessage::DispRetAddShareConst,
        NodeMessage::DispRetOutputReqConst,
        NodeMessage::DispRetProposeConst,
        NodeMessage::DispRetRetrieveRequestConst,
        NodeMessage::DispRetOutputReqConst,
    ]);
    let index = node.lock().await.uindex();
    let mut saw = HashSet::new();
    let mut decoders = Vec::new();
    let mut results: Vec<Vec<u8>> = Vec::new();
    let mut hash_set = HashesSet::new();
    let mut done = false;
    loop {
        let echo = break_if_over!(receiver);
        match echo {
            NodeMessage::DispRetAddShare(share) => {
                let (sender, new_hash_set, shares, decoders_datas) = share.extract();
                hash_set = new_hash_set;
                if decoders.is_empty() {
                    decoders = decoders_datas
                        .clone()
                        .into_iter()
                        .map(|d| RSDecoder::new(d))
                        .collect()
                }
                if saw.contains(&sender) {
                    continue;
                }
                check(sender, &shares, &hash_set).unwrap();
                saw.insert(sender);

                for (s, d) in shares.into_iter().zip(decoders.iter_mut()) {
                    d.add_recovery_share(sender, &s)
                }
            }
            NodeMessage::DispRetPropose(propose) => {
                let (new_hash_set, shares, decoders) = propose.extract();
                hash_set = new_hash_set;
                let echo = enc!(
                    DisperseRetrieve,
                    DispRetCommand::Echo,
                    Echo::new(index, hash_set.clone(), shares, decoders)
                );
                node.lock().await.broadcast(echo, true).await;
            }
            NodeMessage::DispRetRetrieveRequest(i) => {
                let msg = NodeMessage::DispRetRetrieveOutput(results[i].clone());
                let _ = Node::try_wait_and_send(&node, msg).await;
            }
            NodeMessage::DispRetOutputReq => {
                if done {
                    continue;
                }
                interpolate_remaining_shares(
                    &node,
                    hash_set.clone(),
                    index,
                    &mut results,
                    &mut decoders,
                )
                .await;
                done = true;
                let _ = Node::try_wait_and_send(&node, NodeMessage::DispRetDisperseComplete).await;
            }
            _ => panic!("Unexpected message"),
        }
    }
}
