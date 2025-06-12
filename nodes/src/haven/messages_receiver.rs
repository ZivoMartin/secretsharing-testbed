use super::{
    crypto::{EchoMessage, SendMessage},
    dealer::deal,
    receiver::messages_handler,
};

use crate::{
    break_if_over, create_channels, log,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::HavenCommand, select, Wrapped};
use std::io::Write;

pub async fn listen_at(node: Wrapped<Node>) {
    let channel = NodeMessage::HavenSenderConst;
    let mut receiver = node.lock().await.subscribe(channel);
    let mut handlers = Vec::new();
    create_channels!(node, messages_handler);
    Node::wait_for_channel(&node, NodeMessage::HavenEchoConst).await;
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::HavenSender(bytes_message) => {
                log!(
                    node,
                    "Haven, new message: {:?}",
                    HavenCommand::from(bytes_message[0])
                );
                let node = node.clone();
                handlers.push(select!(HavenCommand, bytes_message, node,
                        Send => send,
                        Echo => echo,
                        Ready => ready,
                ));
            }
            _ => panic!("Unexpected message"),
        }
    }
    for h in handlers {
        h.await.unwrap()
    }
}

pub async fn send(node: Wrapped<Node>, bytes: &[u8]) {
    let msg: SendMessage = dec!(bytes);
    node.lock()
        .await
        .send_message(NodeMessage::HavenSend(msg))
        .await;
}

pub async fn echo(node: Wrapped<Node>, bytes: &[u8]) {
    let msg: EchoMessage = dec!(bytes);
    node.lock()
        .await
        .send_message(NodeMessage::HavenEcho(msg))
        .await;
}

pub async fn ready(node: Wrapped<Node>, bytes: &[u8]) {
    let root: Vec<u8> = dec!(bytes);
    node.lock()
        .await
        .send_message(NodeMessage::HavenReady(root))
        .await;
}

pub async fn haven_share(node: Wrapped<Node>) {
    if node.lock().await.im_dealer() {
        deal(node).await;
    }
}
