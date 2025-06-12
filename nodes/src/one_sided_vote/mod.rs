use global_lib::{
    init_message,
    messages::{NameSpace, OneSidedVoteCommand},
    Wrapped,
};

use crate::{
    break_if_over,
    node::{node::Node, node_message::NodeMessage},
};

pub async fn one_sided_vote_listen(node: Wrapped<Node>) {
    let t = node.lock().await.t();
    let mut ok_count = 0;
    let mut vote_count = 0;
    let mut has_voted = false;
    let mut done = false;
    let mut is_ok = false;
    let enough = 2 * t + 1;

    let channels = [
        NodeMessage::OneSidedVoteSenderConst,
        NodeMessage::OneSidedVoteBroadcastVoteRequestConst,
        NodeMessage::OneSidedVoteBroadcastOkRequestConst,
    ];
    let mut receiver = node.lock().await.subscribe_multiple(&channels);
    loop {
        let msg = break_if_over!(receiver);
        if done {
            continue;
        }
        match msg {
            NodeMessage::OneSidedVoteSender(msg) => match OneSidedVoteCommand::from(msg[0]) {
                OneSidedVoteCommand::Ok => {
                    if has_voted {
                        continue;
                    }
                    ok_count += 1;
                    if ok_count == enough {
                        broadcast_vote(&node).await;
                        has_voted = true;
                    }
                }
                OneSidedVoteCommand::Vote => {
                    vote_count += 1;
                    if !has_voted && vote_count > t {
                        broadcast_vote(&node).await;
                        has_voted = true;
                    }
                    if vote_count >= enough {
                        let msg = NodeMessage::OneSidedVoteOutput;
                        Node::wait_and_send(&node, msg).await;
                        done = true;
                    }
                }
            },
            NodeMessage::OneSidedVoteBroadcastVoteRequest => {
                if !has_voted && vote_count > t {
                    broadcast_vote(&node).await;
                    has_voted = true;
                }
            }
            NodeMessage::OneSidedVoteBroadcastOkRequest => {
                if !is_ok {
                    broadcast_ok(&node).await;
                    is_ok = true;
                }
            }
            _ => panic!("Unexpected message"),
        }
    }
}

async fn broadcast_vote(node: &Wrapped<Node>) {
    let msg = init_message(NameSpace::OneSidedVote, OneSidedVoteCommand::Vote);
    let mut node = node.lock().await;
    node.broadcast(msg, true).await;
}

async fn broadcast_ok(node: &Wrapped<Node>) {
    let msg = init_message(NameSpace::OneSidedVote, OneSidedVoteCommand::Ok);
    let mut node = node.lock().await;
    node.broadcast(msg, true).await;
}
