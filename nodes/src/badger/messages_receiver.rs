use super::{broadcast_recv::BroadcastReceiv, dealer::deal, receivers::decrypt_shares};
use crate::{
    break_if_over,
    crypto::Share,
    log,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::BadgerCommand, select, Step, Wrapped};
use std::io::Write;

pub async fn listen_at(node: Wrapped<Node>) {
    let channel = NodeMessage::BadgerSenderConst;
    let mut receiver = node.lock().await.subscribe(channel);
    broadcast_reception_handler(node.clone());
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::BadgerSender(bytes_message) => {
                let node = node.clone();
                select!(
                    BadgerCommand, bytes_message, node,
                    ReconstructShare => new_reconstruct_share
                );
            }
            _ => panic!("Unexpected message"),
        }
    }
}

pub async fn badger_share(node: Wrapped<Node>) {
    log!(node, "Sharing..");
    if node.lock().await.im_dealer() {
        log!(node, "Im the dealer, preparing to share");
        deal(node).await;
    } else {
        log!(node, "Im not the dealer, doing nothing yet.");
    }
}

async fn new_reconstruct_share(node: Wrapped<Node>, bytes: &[u8]) {
    let share: Share = dec!(bytes);
    let _ = Node::wait_and_send(&node, NodeMessage::BadgerReconstructShare(share)).await;
}

fn broadcast_reception_handler(node: Wrapped<Node>) {
    tokio::spawn(async move {
        if node.lock().await.step() == Step::Reconstruct {
            return;
        }
        log!(node, "Will wait for broadcast to complete");
        let channel = NodeMessage::BroadcastBadgerConst;
        let mut receiver = node.lock().await.subscribe(channel);
        log!(node, "Waiting for broadcast...");
        let broadcast_recv: BroadcastReceiv = match receiver.recv().await.unwrap() {
            NodeMessage::BroadcastBadger(b) => dec!(b),
            _ => panic!("Unexpected message"),
        };
        log!(node, "Broadcast phase has ended, decrypting shares.");
        decrypt_shares(node, broadcast_recv).await
    });
}
