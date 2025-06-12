use global_lib::{enc, messages::BingoCommand, wrap, Wrapped};
use notifier_hub::notifier::MessageReceiver as Receiver;

use std::collections::HashMap;

use crate::{
    break_if_over,
    crypto::{interpolate_on_zero, kzg_interpolate_specific_share, Share},
    node::{node::Node, node_message::NodeMessage},
};

const SECRET_TO_RECONSTRUCT: usize = 0;

pub async fn reconstruct(node: Wrapped<Node>) {
    let cloned_node = node.clone();
    let mut node = node.lock().await;
    node.log("Computing share");
    let receiver = node.subscribe(NodeMessage::BingoReconstructShareConst);
    if node.is_byz() {
        return;
    }
    let share_index = node.config().index_secret(SECRET_TO_RECONSTRUCT);
    let mut share = kzg_interpolate_specific_share(node.dom(), &node.shares_vec(), share_index);
    share.set_index(node.index());
    let msg = enc!(Bingo, BingoCommand::ReconstructShare, share);
    tokio::spawn(async move { reconstruct_share_receiver(cloned_node, receiver, share).await });
    node.broadcast(msg, false).await;
    node.log("End of broadcast");
}

#[allow(clippy::needless_if)]
async fn reconstruct_share_receiver(
    node: Wrapped<Node>,
    mut receiver: Receiver<NodeMessage>,
    my_share: Share,
) {
    let enough = node.lock().await.t() + 1;
    let share_index = node
        .lock()
        .await
        .config()
        .index_secret(SECRET_TO_RECONSTRUCT);

    let set = wrap!(HashMap::<u16, Share>::new());
    set.lock().await.insert(my_share.index(), my_share);
    loop {
        let msg = break_if_over!(receiver);
        if set.lock().await.len() == enough as usize {
            continue;
        }
        let set = set.clone();
        let node = node.clone();
        let comm = node.lock().await.get_comm().clone();
        tokio::spawn(async move {
            match msg {
                NodeMessage::BingoReconstructShare(share) => {
                    let i = share.index();
                    assert!(
                        !set.lock().await.contains_key(&i),
                        "I got the same share two times: {i}"
                    );
                    assert!(
                        comm.verify_on(i as usize, share_index, &share.get(0)),
                        "I failed to verify a share: {i}"
                    );
                    let mut set = set.lock().await;
                    set.insert(i, share);
                    node.lock().await.log(&format!("New share: {}", set.len()));
                    if set.len() as u16 == enough {
                        {
                            let node = node.lock().await;
                            let secret = interpolate_on_zero(node.config(), &set);
                            let secrets = node.get_secrets();
                            if secrets.is_some() && !secrets.as_ref().unwrap().contains(&secret) {}
                            // TODO: Handle the interpolation fail
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
