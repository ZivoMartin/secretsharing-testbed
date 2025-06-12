use global_lib::{dec, Wrapped};

use crate::{
    break_if_over,
    crypto::{
        crypto_lib::vss::common::low_deg_test, interpolate_specific_share, Commitment, Share,
    },
    node::{node::Node, node_message::NodeMessage},
    secure_message_dist::ForwardTag,
};

pub async fn wait_for_share(node: Wrapped<Node>) {
    let channel = NodeMessage::SMDOutputConst;
    let mut message_dis_output = node.lock().await.subscribe(channel);
    let share = message_dis_output.recv().await.unwrap();
    if let NodeMessage::SMDOutput(bytes) = share {
        let share: Share = dec!(bytes);
        node.lock().await.save_share(share).await;
        wait_from_comm(&node).await;
        process(node).await
    } else {
        panic!()
    }
}

pub async fn wait_from_comm(node: &Wrapped<Node>) {
    let channel = NodeMessage::BroadcastLightWeightConst;
    let mut comm_receiver = node.lock().await.subscribe(channel);
    let comm = match comm_receiver.recv().await.unwrap() {
        NodeMessage::BroadcastLightWeight(b) => b,
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
    let (happy, mut one_sided_vote, is_byz, i) = {
        let mut node = node.lock().await;
        (
            node.get_comm().verify(node.my_share()),
            node.subscribe(NodeMessage::OneSidedVoteOutputConst),
            node.is_byz(),
            node.uindex(),
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
            node.lock().await.throw_my_share();
            Node::forward(&node, ForwardTag::Complaint(i)).await;
        }
    }
    Node::wait_and_send(&node, NodeMessage::LightWeightEndOfProcessing).await;
}

pub async fn complaint_manager(node: Wrapped<Node>) {
    if !node.lock().await.is_dealer_corrupted() {
        return;
    }

    let kind = NodeMessage::LightWeightEndOfProcessingConst;
    let mut ok_receiver = node.lock().await.subscribe(kind);
    ok_receiver.recv().await;

    let (i, mut receiver) = {
        let mut node = node.lock().await;
        if !node.has_share() {
            return;
        }
        node.kill_channel(NodeMessage::SMDForwardLightWeightAssistConst);
        let msg_kind = NodeMessage::SMDForwardLightWeightComplaintConst;
        (node.uindex(), node.subscribe(msg_kind))
    };

    let complaint = match receiver.recv().await {
        Some(complaint) => complaint,
        None => return,
    };

    match complaint {
        NodeMessage::SMDForwardLightWeightComplaint(mem, msg) => {
            let bytes = mem.lock().await.forward_decode(msg);
            let share: Share = dec!(bytes);
            if node.lock().await.get_comm().verify(&share) {
                panic!("The complaint is invalid")
            }
            Node::forward(&node, ForwardTag::Assist(i)).await;
            Node::forward(&node, ForwardTag::Report(share.uindex())).await;
            Node::output(node);
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

    let msg_kind = NodeMessage::SMDForwardLightWeightAssistConst;
    let (enough, mut receiver) = {
        let mut node = node.lock().await;
        if node.has_share() {
            return;
        }
        node.kill_channel(NodeMessage::SMDForwardLightWeightComplaintConst);
        (node.t() as usize + 1, node.subscribe(msg_kind))
    };

    let mut shares = Vec::with_capacity(enough);
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::SMDForwardLightWeightAssist(mem, msg) => {
                let bytes = mem.lock().await.forward_decode(msg);
                let share: Share = dec!(bytes);
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

pub async fn report_manager(node: Wrapped<Node>) {
    if !node.lock().await.is_dealer_corrupted() {
        return;
    }

    let kind = NodeMessage::LightWeightEndOfProcessingConst;
    let mut ok_receiver = node.lock().await.subscribe(kind);
    ok_receiver.recv().await;

    let msg_kind = NodeMessage::SMDForwardLightWeightReportConst;
    let mut receiver = node.lock().await.subscribe(msg_kind);
    loop {
        let msg = break_if_over!(receiver);
        match msg {
            NodeMessage::SMDForwardLightWeightReport(mem, msg) => {
                let bytes = mem.lock().await.forward_decode(msg);
                let share: Share = dec!(bytes);
                assert!(!node.lock().await.get_comm().verify(&share));
            }
            _ => panic!(),
        }
    }
}
