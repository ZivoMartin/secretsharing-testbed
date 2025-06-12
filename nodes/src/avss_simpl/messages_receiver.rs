use super::{
    dealer::deal,
    receiver::{first_receiv, verify_and_output},
};
use crate::{
    avss_simpl::crypto_messages::BroadcastReceiv,
    break_if_over,
    crypto::{Commitment, Share},
    log,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::AvssSimplCommand, select, Step, Wrapped};
use std::io::Write;

pub async fn listen_at(node: Wrapped<Node>) {
    let channel = NodeMessage::AvssSimplSenderConst;
    let mut receiver = node.lock().await.subscribe(channel);
    broadcast_reception_handler(node.clone()).await;
    let mut handlers = Vec::new();
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::AvssSimplSender(bytes_message) => {
                log!(
                    node,
                    "Avss Simpl, new message: {:?}",
                    AvssSimplCommand::from(bytes_message[0])
                );
                let node = node.clone();
                handlers.push(select!(AvssSimplCommand, bytes_message, node,
                        Share => share_receiv,
                        Ack => new_sign,
                        NewShare => new_share,
                ));
            }
            _ => panic!("Unexpected message"),
        }
    }
    for h in handlers {
        h.await.unwrap()
    }
}

pub async fn avss_simpl_share(node: Wrapped<Node>) {
    if node.lock().await.im_dealer() {
        let channel = NodeMessage::AvssSimplDealerMessageConst;
        let receiver = node.lock().await.subscribe(channel);
        deal(node, receiver).await;
    }
}

async fn share_receiv(node: Wrapped<Node>, bytes: &[u8]) {
    let (share, comm): (Share, Commitment) = dec!(bytes);
    first_receiv(node, comm, share).await;
}

async fn new_sign(node: Wrapped<Node>, bytes: &[u8]) {
    let (i, sign) = dec!(bytes, (u16, Sign));
    let _ = Node::try_wait_and_send(&node, NodeMessage::AvssSimplDealerMessage(i, sign)).await;
}

async fn new_share(node: Wrapped<Node>, bytes: &[u8]) {
    let msg = NodeMessage::AvssSimplExtShare(dec!(bytes, Share));
    Node::wait_and_send(&node, msg).await;
}

async fn broadcast_reception_handler(node: Wrapped<Node>) {
    node.clone()
        .lock()
        .await
        .push_handler(tokio::spawn(async move {
            if node.lock().await.step() == Step::Reconstruct {
                return;
            }
            let channel = NodeMessage::BroadcastAvssSimplConst;
            let mut receiver = node.lock().await.subscribe(channel);
            let broadcast_recv: BroadcastReceiv = match receiver.recv().await.unwrap() {
                NodeMessage::BroadcastAvssSimpl(b) => dec!(b),
                _ => panic!("Unexpected message"),
            };
            verify_and_output(node, broadcast_recv).await
        }));
}
