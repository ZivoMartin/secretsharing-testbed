use super::{
    dealer::deal,
    receivers::{assist_manager, complaint_manager, report_manager, wait_for_share},
};
use crate::{
    create_channels,
    node::{node::Node, node_message::NodeMessage},
};

use global_lib::Wrapped;

pub async fn listen_at(node: Wrapped<Node>) {
    let channel = NodeMessage::LightWeightSenderConst;
    let mut channel = node.lock().await.subscribe(channel);

    tokio::spawn(async move { wait_for_share(node).await });
    channel.recv().await;
}

pub async fn lightweight_share(node: Wrapped<Node>) {
    init_channels(&node).await;
    if node.lock().await.im_dealer() {
        deal(node).await;
    }
}

async fn init_channels(node: &Wrapped<Node>) {
    create_channels!(node, complaint_manager, assist_manager, report_manager);
}
