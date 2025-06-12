use crate::{
    break_if_over, log,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::BroadcastCommand, select, wrap, Wrapped};
use std::io::Write;

use super::{
    broadcast_memory::BroadcastMemory,
    broadcast_message_types::{BroadcastMessageType, Transcript},
};

pub async fn listen(node: Wrapped<Node>) {
    let memory = wrap!(BroadcastMemory::new(node.clone()));
    let mut receiver = node
        .lock()
        .await
        .subscribe(NodeMessage::BroadcastSenderConst);
    let mut handlers = Vec::new();
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::BroadcastSender(mut bytes_message) => {
                let memory = memory.clone();
                log!(
                    node,
                    "Broadcast: New Message, {:?}",
                    BroadcastCommand::from(bytes_message[0])
                );
                handlers.push(select!(
                    as_vec
                    BroadcastCommand, bytes_message, memory,
                    Propose => propose,
                    Echo => new_echo,
                    Ready => new_ready
                ));
            }
            _ => panic!("Unexpected message"),
        }
    }
    for h in handlers {
        h.await.unwrap()
    }
}

async fn propose(memory: Wrapped<BroadcastMemory>, mut bytes: Vec<u8>) {
    let kind = BroadcastMessageType::from(bytes.remove(0));
    memory.lock().await.propose(kind, bytes).await;
}

async fn new_echo(memory: Wrapped<BroadcastMemory>, bytes: Vec<u8>) {
    let tr: Transcript = dec!(bytes);
    memory.lock().await.add_echo(tr).await;
}

async fn new_ready(memory: Wrapped<BroadcastMemory>, bytes: Vec<u8>) {
    let tr: Transcript = dec!(bytes);
    memory.lock().await.add_ready(tr).await;
}
