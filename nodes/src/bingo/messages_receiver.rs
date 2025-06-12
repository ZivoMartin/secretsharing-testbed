use super::{
    dealer::deal,
    receivers::{col_manager, done_manager, line_manager, verify_my_line},
};
use crate::{
    break_if_over, create_channels,
    crypto::{Polynomial, Share},
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::BingoCommand, select, Step, Wrapped};

pub async fn listen_at(node: Wrapped<Node>) {
    let mut receiver = node.lock().await.subscribe(NodeMessage::BingoSenderConst);
    if node.lock().await.step() == Step::Sharing {
        let channel = NodeMessage::BroadcastBingoConst;
        let mut comm_receiver = node.lock().await.subscribe(channel);
        let comm = match comm_receiver.recv().await.unwrap() {
            NodeMessage::BroadcastBingo(bytes) => dec!(bytes),
            _ => panic!("Unexpected message"),
        };
        node.lock().await.set_comm(comm);
    }

    create_channels!(node, col_manager, line_manager, done_manager);
    Node::wait_for_channel(&node, NodeMessage::BingoColConst).await;
    Node::wait_for_channel(&node, NodeMessage::BingoRowConst).await;
    Node::wait_for_channel(&node, NodeMessage::BingoDoneConst).await;

    let mut handlers = Vec::new();
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::BingoSender(bytes_message) => {
                let node = node.clone();
                handlers.push(select!(
                    BingoCommand, bytes_message, node,
                    MyLine => my_line,
                    NewRow => new_row,
                    NewCol => new_col,
                    NewDone => new_done,
                    ReconstructShare => new_reconstruct_share
                ));
            }
            _ => panic!("Unexpected message"),
        }
    }
    for h in handlers {
        h.await.unwrap()
    }
}

pub async fn bingo_share(node: Wrapped<Node>) {
    if node.lock().await.im_dealer() {
        deal(node).await;
    }
}

async fn my_line(node: Wrapped<Node>, bytes: &[u8]) {
    let line: Polynomial = dec!(bytes);
    verify_my_line(node, line).await;
}

async fn new_row(node: Wrapped<Node>, bytes: &[u8]) {
    let share: Share = dec!(bytes, Share);
    let _ = node
        .lock()
        .await
        .try_send_message(NodeMessage::BingoRow(share))
        .await;
}

async fn new_col(node: Wrapped<Node>, bytes: &[u8]) {
    let share: Share = dec!(bytes);
    let _ = node
        .lock()
        .await
        .try_send_message(NodeMessage::BingoCol(share))
        .await;
}

async fn new_done(node: Wrapped<Node>, _bytes: &[u8]) {
    let _ = node
        .lock()
        .await
        .try_send_message(NodeMessage::BingoDone)
        .await;
}

async fn new_reconstruct_share(node: Wrapped<Node>, bytes: &[u8]) {
    let msg = NodeMessage::BingoReconstructShare(dec!(bytes, Share));
    let _ = node.lock().await.try_send_message(msg).await;
}
