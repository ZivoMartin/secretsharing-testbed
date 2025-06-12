use crate::{
    broadcast::broadcast_message_types::BroadcastMessageType,
    crypto::{compute_comm_and_shares, Share},
    node::node::Node,
};
use global_lib::{enc, Wrapped};
use std::collections::VecDeque;

pub async fn deal(node: Wrapped<Node>) {
    let mut node = node.lock().await;
    let (comm, mut output, secrets) = compute_comm_and_shares(node.config());
    node.set_secrets(secrets);
    let n = node.n() as usize;
    let cor = n - node.dealer_corruption() as usize;
    output.truncate(cor);
    let empty_share = Share::empty(node.batch_size() as usize);
    let mut messages = VecDeque::with_capacity(n);
    let mut i = n;
    while i > 0 {
        i -= 1;
        let share = if i >= cor {
            let mut share = empty_share.clone();
            share.set_index(i as u16);
            share
        } else {
            output.pop().unwrap()
        };
        let buf = enc!(share);
        messages.push_front(buf)
    }
    node.distribute(messages.into_iter().collect()).await;
    node.reliable_broadcast(BroadcastMessageType::LightWeight, enc!(comm))
        .await
}
