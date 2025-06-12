use super::{dealer::deal, receivers::wait_for_share, HbAvssAssist, HbAvssComplaint};
use crate::{
    break_if_over, create_channels,
    hbavss::receivers::{assist_manager, complaint_manager},
    node::{node::Node, node_message::NodeMessage},
};
use global_lib::{dec, messages::HbAvssCommand, select, Wrapped};

pub async fn listen_at(node: Wrapped<Node>) {
    let mut receiver = node.lock().await.subscribe(NodeMessage::HbAvssSenderConst);

    let cloned_node = node.clone();
    tokio::spawn(async move { wait_for_share(cloned_node).await });

    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::HbAvssSender(bytes_message) => {
                let node = node.clone();
                select!(
                    HbAvssCommand, bytes_message, node,
                    Complaint => new_complaint,
                    Assist => new_assist,
                );
            }
            _ => panic!("Unexpected message"),
        }
    }
}

pub async fn hbavss_share(node: Wrapped<Node>) {
    init_channels(&node).await;
    if node.lock().await.im_dealer() {
        deal(node).await;
    }
}

async fn new_complaint(node: Wrapped<Node>, bytes: &[u8]) {
    let complaint: HbAvssComplaint = dec!(bytes);
    Node::wait_and_send(&node, NodeMessage::HbAvssComplaint(complaint)).await;
}

async fn new_assist(node: Wrapped<Node>, bytes: &[u8]) {
    let assist: HbAvssAssist = dec!(bytes);
    Node::wait_and_send(&node, NodeMessage::HbAvssAssist(assist)).await;
}

async fn init_channels(node: &Wrapped<Node>) {
    create_channels!(node, complaint_manager, assist_manager);
}
