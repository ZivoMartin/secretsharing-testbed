use crate::{
    break_if_over, create_channels,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::DispRetCommand, select, Wrapped};

use super::memory::{echo_manager, messages_handler, ready_manager};

pub async fn listen(node: Wrapped<Node>) {
    let channel = NodeMessage::DispRetSenderConst;

    create_channels!(node, messages_handler, echo_manager, ready_manager);
    let mut receiver = node.lock().await.subscribe(&channel);
    let mut handlers = Vec::new();
    loop {
        let msg = break_if_over!(receiver);
        let node = node.clone();
        match msg {
            NodeMessage::DispRetSender(mut bytes_message) => {
                handlers.push(select!(
                    as_vec
                    DispRetCommand, bytes_message, node,
                    Propose => propose,
                    Ready => new_ready,
                    Echo => new_echo
                ));
            }
            _ => panic!("Unexpected message"),
        }
    }
    for h in handlers {
        h.await.unwrap()
    }
}

async fn new_ready(node: Wrapped<Node>, bytes: Vec<u8>) {
    let ready = NodeMessage::DispRetReady(dec!(bytes));
    let _ = node.lock().await.try_send_message(ready).await;
}

async fn new_echo(node: Wrapped<Node>, bytes: Vec<u8>) {
    let echo = NodeMessage::DispRetEcho(dec!(bytes));
    let _ = node.lock().await.try_send_message(echo).await;
}

async fn propose(node: Wrapped<Node>, bytes: Vec<u8>) {
    let propose = NodeMessage::DispRetPropose(dec!(bytes));
    let _ = node.lock().await.try_send_message(propose).await;
}
