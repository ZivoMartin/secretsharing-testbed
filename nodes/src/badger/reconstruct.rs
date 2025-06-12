use global_lib::{enc, messages::BadgerCommand, wrap, Wrapped};
use notifier_hub::notifier::MessageReceiver as Receiver;
use std::collections::HashMap;

use crate::{
    break_if_over,
    crypto::{interpolate, Share},
    node::{node::Node, node_message::NodeMessage},
};

pub async fn reconstruct(node: Wrapped<Node>) {
    let cloned_node = node.clone();
    let mut node = node.lock().await;
    let receiver = node.subscribe(NodeMessage::BadgerReconstructShareConst);
    tokio::spawn(async move { reconstruct_share_receiver(cloned_node, receiver).await });
    if node.is_byz() {
        return;
    }
    let msg = enc!(Badger, BadgerCommand::ReconstructShare, node.my_share());
    node.broadcast(msg, false).await;
}

async fn reconstruct_share_receiver(node: Wrapped<Node>, mut receiver: Receiver<NodeMessage>) {
    let my_share = node.lock().await.my_share().clone();
    let enough = node.lock().await.l() + 1;
    let mut share_counter = enough - 1;
    let set = wrap!(HashMap::<u16, Share>::new());
    set.lock().await.insert(my_share.index(), my_share);
    loop {
        let msg = break_if_over!(receiver);
        if share_counter == 0 {
            continue;
        }
        share_counter -= 1;
        let set = set.clone();
        let node = node.clone();
        let comm = node.lock().await.get_comm().clone();
        tokio::spawn(async move {
            match msg {
                NodeMessage::BadgerReconstructShare(share) => {
                    let i = share.index();
                    if !comm.verify_on(0, i as usize, &share.get(0))
                        || set.lock().await.contains_key(&i)
                    {
                        panic!("I Failed to recv a share")
                    }
                    let mut set = set.lock().await;
                    set.insert(i, share);
                    if set.len() as u16 == enough {
                        {
                            let node = node.lock().await;
                            if !interpolate(node.config(), &set, node.get_secrets()) {
                                panic!("FAILED TO INTERPOLATE")
                            }
                        }
                        Node::output(node);
                    }
                }
                _ => {
                    panic!("Unexpected message")
                }
            }
        });
    }
}
