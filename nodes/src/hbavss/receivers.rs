use blsttc::serde_impl::SerdeSecret;
use global_lib::{dec, enc, messages::HbAvssCommand, Wrapped};

use crate::{
    break_if_over,
    crypto::{
        crypto_lib::vss::common::low_deg_test, interpolate_specific_share, Commitment, Share,
    },
    hbavss::HbAvssAssist,
    node::{node::Node, node_message::NodeMessage},
};

use super::HbAvssComplaint;

pub async fn wait_for_share(node: Wrapped<Node>) {
    let channel = NodeMessage::DispRetDisperseCompleteConst;
    let mut disperse_output = node.lock().await.subscribe(channel);
    if let NodeMessage::DispRetDisperseComplete = disperse_output.recv().await.unwrap() {
        let (index, key) = {
            let node = node.lock().await;
            (node.uindex(), node.my_blstt_skey().clone())
        };
        let bytes = Node::retrieve(&node, index, Some(key)).await;
        let share: Share = dec!(bytes);
        node.lock().await.save_share(share).await;

        wait_from_comm(&node).await;
        process(node).await
    } else {
        panic!()
    }
}

pub async fn wait_from_comm(node: &Wrapped<Node>) {
    let channel = NodeMessage::BroadcastHbAvssConst;
    let mut comm_receiver = node.lock().await.subscribe(channel);
    let comm = match comm_receiver.recv().await.unwrap() {
        NodeMessage::BroadcastHbAvss(b) => b,
        _ => panic!("Lightweight broadcast was expected"),
    };
    let comm: Commitment = dec!(comm);
    let mut node = node.lock().await;
    if !low_deg_test(&comm, node.config()) {
        panic!("Low deg test didn't pass");
    }
    node.set_comm(comm);
}

async fn process(node: Wrapped<Node>) {
    let (happy, mut one_sided_vote, is_byz, index) = {
        let mut node = node.lock().await;
        (
            node.get_comm().verify(node.my_share()),
            node.subscribe(NodeMessage::OneSidedVoteOutputConst),
            node.is_byz(),
            node.index(),
        )
    };

    if happy && !is_byz {
        let msg = NodeMessage::OneSidedVoteBroadcastOkRequest;
        node.lock().await.send_message(msg).await;
    }

    one_sided_vote.recv().await.unwrap();
    {
        if happy {
            if !node.lock().await.is_dealer_corrupted() {
                Node::output(node);
                return;
            }
        } else {
            let complaint = enc!(
                HbAvss,
                HbAvssCommand::Complaint,
                HbAvssComplaint {
                    index,
                    pkey: SerdeSecret(node.lock().await.my_blstt_skey().clone()),
                }
            );
            node.lock().await.broadcast(complaint, false).await;
        }
    }
    Node::wait_and_send(&node, NodeMessage::HbAvssEndOfProcessing).await;
}

pub async fn complaint_manager(node: Wrapped<Node>) {
    if !node.lock().await.is_dealer_corrupted() {
        return;
    }

    let kind = NodeMessage::HbAvssEndOfProcessingConst;
    let mut ok_receiver = node.lock().await.subscribe(kind);
    ok_receiver.recv().await;

    let (mut receiver, index) = {
        let mut node = node.lock().await;
        if !node.has_share() {
            return;
        }
        node.kill_channel(NodeMessage::HbAvssAssistConst);
        let msg_kind = NodeMessage::HbAvssComplaintConst;
        (node.subscribe(msg_kind), node.index())
    };

    let complaint = match receiver.recv().await {
        Some(complaint) => complaint,
        None => return,
    };

    match complaint {
        NodeMessage::HbAvssComplaint(complaint) => {
            let share =
                Node::retrieve(&node, complaint.index as usize, Some(complaint.pkey.0)).await;
            let share: Share = dec!(share);
            assert!(!node.lock().await.get_comm().verify(&share));
            let complaint = enc!(
                HbAvss,
                HbAvssCommand::Assist,
                HbAvssAssist {
                    index,
                    pkey: SerdeSecret(node.lock().await.my_blstt_skey().clone()),
                }
            );
            node.lock().await.broadcast(complaint, false).await;

            Node::output(node.clone());
        }
        NodeMessage::Close => receiver.close(),
        _ => panic!("A Complaint was expected.."),
    }
    loop {
        break_if_over!(receiver);
    }
}

pub async fn assist_manager(node: Wrapped<Node>) {
    if !node.lock().await.is_dealer_corrupted() {
        return;
    }
    let kind = NodeMessage::LightWeightEndOfProcessingConst;
    let mut ok_receiver = node.lock().await.subscribe(kind);
    ok_receiver.recv().await;

    let (enough, mut receiver) = {
        let mut node = node.lock().await;
        if node.has_share() {
            return;
        }
        node.kill_channel(NodeMessage::HbAvssComplaintConst);
        let msg_kind = NodeMessage::HbAvssAssistConst;
        (node.t() as usize + 1, node.subscribe(msg_kind))
    };

    let mut shares = Vec::with_capacity(enough);
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::HbAvssAssist(assist) => {
                let share = Node::retrieve(&node, assist.index as usize, Some(assist.pkey.0)).await;
                let share: Share = dec!(share);
                assert!(node.lock().await.get_comm().verify(&share));

                let mut lnode = node.lock().await;
                if lnode.get_comm().verify(&share) {
                    shares.push(share);
                    if shares.len() == enough {
                        shares.sort();
                        let share =
                            interpolate_specific_share(lnode.dom(), &shares, lnode.uindex());
                        lnode.save_share(share).await;
                        Node::output(node.clone());
                    }
                } else {
                    eprintln!(
                        "Failed to verify the share {} with the node {} (row)",
                        lnode.index(),
                        share.index()
                    )
                }
            }
            _ => panic!("A done message was expected"),
        }
    }
}
