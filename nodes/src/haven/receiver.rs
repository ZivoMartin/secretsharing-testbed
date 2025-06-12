use super::crypto::{EchoMessage, SendMessage};
use crate::{
    break_if_over,
    crypto::{scheme::interpolate_specific_share, Commitment, Share},
    log,
    node::{node::Node, node_message::NodeMessage},
};
use blstrs::Scalar;
use global_lib::{enc, messages::HavenCommand, Wrapped};
use std::io::Write;

async fn interpolate_and_output(node: Wrapped<Node>, shares: Vec<(usize, Vec<(Scalar, Scalar)>)>) {
    {
        let mut node = node.lock().await;
        let mut share = interpolate_specific_share(
            node.dom(),
            &shares
                .into_iter()
                .map(|(sender, shares)| {
                    let (shares, rands) = shares.into_iter().unzip();
                    Share::new(sender as u16, shares, rands)
                })
                .collect::<Vec<_>>(),
            node.uindex() + 1,
        );
        share.set_index(node.index());
        node.save_share(share).await;
    }
    Node::output(node);
}

pub async fn messages_handler(node: Wrapped<Node>) {
    {
        let mut node = node.lock().await;
        let base = *node.config().base();
        node.set_comm(Commitment::new(base));
    }

    let mut receiver = node.lock().await.subscribe_multiple(&[
        NodeMessage::HavenSendConst,
        NodeMessage::HavenReadyConst,
        NodeMessage::HavenEchoConst,
    ]);
    let (n, t, b, index) = {
        let node = node.lock().await;
        (
            node.config().n() as usize,
            node.t(),
            node.batch_size(),
            node.uindex(),
        )
    };

    let mut shares = Vec::new();
    let mut ready = false;
    let mut output = false;
    let mut echo_count = 0;
    let mut ready_count = 0;
    let enough = 2 * t + 1;

    loop {
        let msg = break_if_over!(receiver);
        if output {
            continue;
        }
        match msg {
            NodeMessage::HavenSend(SendMessage {
                root,
                comms: (comms, _),
                evals,
            }) => {
                log!(node, "Just received message Send");
                let mut shares = vec![Vec::new(); n];
                for i in 0..b as usize {
                    for (eval, comm) in evals.iter().zip(comms.iter()) {
                        let [s, r] = eval.get(i);
                        comm.verify_on(i, index, &[s, r]);
                        log!(node, "Successfully verif share {i}");
                        shares[i].push((s, r))
                    }
                }
                log!(node, "Successfully verif all shares");
                let mut node = node.lock().await;
                for (i, (evals, comm)) in
                    (shares.into_iter().zip(comms.into_iter()).enumerate()).rev()
                {
                    let msg = enc!(
                        Haven,
                        HavenCommand::Echo,
                        EchoMessage {
                            root: root.clone(),
                            sender: index,
                            comm,
                            evals
                        }
                    );
                    node.give_contact(i, msg)
                }
            }
            NodeMessage::HavenReady(root) => {
                log!(node, "Received new ready");
                ready_count += 1;
                if !ready && ready_count == t + 1 {
                    log!(node, "Just received enough share, sending ready");
                    ready = true;
                    node.lock()
                        .await
                        .broadcast(enc!(Haven, HavenCommand::Ready, root), true)
                        .await;
                } else if ready_count == enough {
                    if echo_count >= t + 1 {
                        log!(node, "Outputing (ready path)");
                        output = true;
                        interpolate_and_output(node.clone(), shares.drain(..).collect()).await;
                    }
                }
            }
            NodeMessage::HavenEcho(EchoMessage {
                root,
                comm,
                sender,
                evals,
            }) => {
                if ready_count < enough && echo_count > enough {
                    continue;
                }
                log!(node, "Just received an echo message from {sender}");
                for (i, (e, r)) in evals.iter().enumerate() {
                    assert!(comm.verify_on(i, sender, &[*e, *r]))
                }
                shares.push((sender, evals));
                log!(node, "Successfully verified the share of  {sender}");
                echo_count += 1;
                log!(
                    node,
                    "Actualising echo_count to {echo_count}, goal is : {enough}"
                );
                if !ready && echo_count == enough {
                    log!(node, "Just received enough share, sending ready");
                    ready = true;
                    node.lock()
                        .await
                        .broadcast(enc!(Haven, HavenCommand::Ready, root), true)
                        .await;
                } else if echo_count == t + 1 && ready {
                    log!(node, "Outputing (echo path)");
                    output = true;

                    interpolate_and_output(node.clone(), shares.drain(..).collect()).await;
                }
            }
            _ => unreachable!(),
        }
    }
}
