use crate::{
    broadcast::broadcast_message_types::BroadcastMessageType,
    crypto::{
        crypto_lib::{crypto_blstrs::poly_commit::kzg::BlstrsKZG, random_scalars},
        data_structures::commitment::Commitment,
        BiVariatePoly,
    },
    node::node::Node,
};
use futures::stream::{self, StreamExt};
use global_lib::{enc, messages::BingoCommand, Wrapped};
use rand::thread_rng;
use std::sync::Arc;

pub async fn deal(node: Wrapped<Node>) {
    let mut node = node.lock().await;
    node.log("AS DEALER: Bingo deal beggin");
    let n = node.n() as usize;
    let t = node.t() as usize;
    let (secrets, bi, mut comm) = {
        let rng = &mut thread_rng();
        let secrets = random_scalars(node.config().batch_size() as usize, rng);
        let bi = BiVariatePoly::random_with_rands(secrets.clone(), 2 * t, t, node.config(), rng);
        (
            secrets,
            Arc::new(bi),
            Commitment::new_kzg(rng, 2 * node.config().degree() as usize),
        )
    };
    if node.im_dealer() {
        node.set_secrets(secrets);
    }
    let mut lines = Vec::with_capacity(n);
    let dom = Arc::new(node.dom().clone());
    let corruption = node.config().dealer_corruption() as usize;
    let setup = comm.kzg_setup();
    let mut datas = stream::iter(1..=n)
        .map(|i| {
            let dom = dom.clone();
            let bi = bi.clone();
            let setup = setup.clone();
            tokio::spawn(async move {
                let r = dom.get_root_of_unity(i);
                let ax = bi.evaluate_on_y(r);
                (i, BlstrsKZG::commit(&setup, &ax).unwrap(), ax)
            })
        })
        .buffer_unordered(10)
        .map(|res| res.unwrap())
        .collect::<Vec<_>>()
        .await;
    datas.sort_by_key(|&(index, _, _)| index);

    for (_, c, ax) in datas {
        comm.kzg_push(c);
        lines.push(ax)
    }

    let bytes = enc!(comm);
    node.reliable_broadcast(BroadcastMessageType::Bingo, bytes)
        .await;
    for (i, ax) in (0..n - corruption).zip(lines) {
        let msg = enc!(Bingo, BingoCommand::MyLine, ax);
        node.contact(i, Arc::new(msg));
    }
}
