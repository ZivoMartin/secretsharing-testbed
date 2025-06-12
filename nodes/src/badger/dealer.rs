use crate::{
    broadcast::broadcast_message_types::BroadcastMessageType,
    crypto::{compute_comm_and_shares, encode_shares},
    node::node::Node,
};
use global_lib::{enc, Wrapped};

use super::broadcast_recv::BroadcastReceiv;

pub async fn deal(node: Wrapped<Node>) {
    let mut node = node.lock().await;
    let (comm, shares, secrets) = compute_comm_and_shares(node.config());
    node.set_secrets(secrets);
    let encs = encode_shares(node.config(), &comm, &node.get_all_pkey(), &shares).await;
    let msg = enc!(BroadcastReceiv { encs, comm });
    node.reliable_broadcast(BroadcastMessageType::Badger, msg)
        .await;
}
