use super::{
    messages::{EchoMessage, ForwardMessage, ForwardTag, ProposeMessage, VoteMessage},
    smd_memory::Memory,
};
use crate::{
    break_if_over,
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::SecureMsgDisCommand, select, wrap, Wrapped};

pub async fn listen(node: Wrapped<Node>) {
    let channels = vec![
        NodeMessage::SMDSenderConst,
        NodeMessage::SMDForwardRequestConst,
    ];
    let memory = wrap!(Memory::new(node.clone()).await);
    let mut receiver = node.lock().await.subscribe_multiple(&channels);
    loop {
        let msg = break_if_over!(receiver);
        let memory = memory.clone();
        match msg {
            NodeMessage::SMDSender(mut bytes_message) => {
                select!(
                    as_vec
                    SecureMsgDisCommand, bytes_message, memory,
                    Propose => propose,
                    Echo => new_echo,
                    Vote => new_vote,
                    Forward => forward_receiv,
                );
            }
            NodeMessage::SMDForwardRequest(tag) => {
                forward_request(memory, tag).await;
            }
            _ => panic!("Unexpected message"),
        }
    }
}

async fn propose(memory: Wrapped<Memory>, bytes: Vec<u8>) {
    let msg: ProposeMessage = dec!(bytes);
    memory.lock().await.propose(msg).await;
}
async fn new_echo(memory: Wrapped<Memory>, bytes: Vec<u8>) {
    let msg: EchoMessage = dec!(bytes);
    memory.lock().await.new_echo(msg).await;
}
async fn new_vote(memory: Wrapped<Memory>, bytes: Vec<u8>) {
    let msg: VoteMessage = dec!(bytes);
    memory.lock().await.new_vote(msg).await;
}

async fn forward_receiv(memory: Wrapped<Memory>, bytes: Vec<u8>) {
    let msg: ForwardMessage = dec!(bytes);
    Memory::forward_receiv(memory, msg).await;
}

async fn forward_request(memory: Wrapped<Memory>, tag: ForwardTag) {
    memory.lock().await.forward_request(tag).await;
}
