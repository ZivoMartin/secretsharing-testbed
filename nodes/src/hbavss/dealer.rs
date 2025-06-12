use crate::{
    broadcast::broadcast_message_types::BroadcastMessageType,
    crypto::{compute_comm_and_shares, Share},
    node::node::Node,
};
use futures::stream::{self, StreamExt};
use global_lib::{enc, Wrapped};

pub async fn deal(node: Wrapped<Node>) {
    let mut node = node.lock().await;
    let (comm, mut output, secrets) = compute_comm_and_shares(node.config());
    node.set_secrets(secrets);
    let n = node.n() as usize;
    let cor = n - node.dealer_corruption() as usize;
    output.truncate(cor);
    let empty_share = Share::empty(node.batch_size() as usize);
    let mut messages: Vec<(usize, Vec<u8>)> = stream::iter((0..n).rev())
        .map(|i| {
            let empty_share = empty_share.clone();
            let keys = node.get_all_pkey();
            let share = if i >= cor {
                let mut share = empty_share.clone();
                share.set_index(i as u16);
                share
            } else {
                output.pop().unwrap()
            };
            tokio::spawn(async move { (i, enc!(keys[i].blstt_encrypt(enc!(share)))) })
        })
        .buffer_unordered(10)
        .map(|res| res.unwrap())
        .collect::<Vec<_>>()
        .await;

    messages.sort_by(|a, b| a.0.cmp(&b.0));

    node.disperse(messages.into_iter().map(|(_, m)| m).collect())
        .await;
    node.reliable_broadcast(BroadcastMessageType::HbAvss, enc!(comm))
        .await
}
