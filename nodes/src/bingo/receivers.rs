use crate::{
    break_if_over,
    crypto::{
        crypto_lib::{crypto_blstrs::poly_commit::kzg::BlstrsKZG, fft::fft},
        kzg_interpolate_all_without_proofs,
        scheme::kzg_interpolate_all,
        Polynomial, Share,
    },
    node::{node::Node, node_message::NodeMessage},
};
use blstrs::Scalar;
use global_lib::{
    enc, init_message,
    messages::{BingoCommand, NameSpace},
    wrap, Wrapped,
};
use std::sync::Arc;

pub async fn verify_my_line(node: Wrapped<Node>, line: Polynomial) {
    let line = Arc::new(line);
    let (base, n, index, is_byz, mut evaluations) = {
        let node = node.lock().await;
        if !node.get_comm().kzg_verify_poly(&line, node.uindex()) {
            eprintln!("Failed to verify my line");
            return;
        }
        (
            node.get_comm().kzg_setup().clone(),
            node.n() as usize,
            node.uindex(),
            node.is_byz(),
            fft(line.fields(), node.config().get_evaluation_domain()),
        )
    };

    evaluations.truncate(n);
    if index % 2 == 0 {
        evaluations = evaluations.into_iter().rev().collect()
    }
    let shares = wrap!(vec!(None; n));

    async fn handle_share_batch(
        begin: usize,
        end: usize,
        evaluations: Vec<Scalar>,
        node: Wrapped<Node>,
        index: usize,
        n: usize,
        line: Arc<Polynomial>,
        is_byz: bool,
        base: Arc<BlstrsKZG>,
        shares: Wrapped<Vec<Option<Share>>>,
    ) {
        assert!(evaluations.len() == end - begin);
        assert!(end <= n);
        for (eval, mut i) in evaluations.into_iter().zip(begin..end) {
            if index % 2 == 0 {
                i = n - i - 1
            }
            let r = node.lock().await.dom().get_root_of_unity(i);
            let w = base.open(&line, eval, &r).unwrap();
            let mut share = Share::kzg_new(index as u16, eval, w);
            if is_byz {
                share.corrupt();
            }
            let mut msg = init_message(NameSpace::Bingo, BingoCommand::NewCol);
            enc!(share, msg);
            node.lock().await.contact(i, Arc::new(msg));
            share.set_index(i as u16);
            let mut shares = shares.lock().await;
            shares[i] = Some(share);
        }
    }

    let nb_threads = 1;
    let nb_share_per_thread = n / nb_threads;
    let add_one_more = n % nb_threads;
    let mut begin = 0;
    let threads = (0..nb_threads)
        .map(|i| {
            let thread_begin = begin;
            let end = begin + nb_share_per_thread + (i < add_one_more) as usize;
            let evaluations: Vec<Scalar> = evaluations.drain(..(end - begin)).collect();
            let node = node.clone();
            let line = line.clone();
            let base = base.clone();
            let shares = shares.clone();
            begin = end;
            tokio::spawn(async move {
                handle_share_batch(
                    thread_begin,
                    end,
                    evaluations,
                    node,
                    index,
                    n,
                    line,
                    is_byz,
                    base,
                    shares,
                )
                .await;
            })
        })
        .collect::<Vec<_>>();

    for t in threads {
        t.await.unwrap();
    }
    node.lock()
        .await
        .send_message(NodeMessage::BingoBroadcastDoneRequest(
            shares.lock().await.drain(..).map(|s| s.unwrap()).collect(),
        ))
        .await;
}

pub async fn col_manager(node: Wrapped<Node>) {
    let mut receiver = node.lock().await.subscribe(NodeMessage::BingoColConst);
    let enough = node.lock().await.t() as usize + 1;
    let i = node.lock().await.index() as usize;
    let mut shares = Vec::with_capacity(enough);
    let comm = node.lock().await.get_comm().clone();
    loop {
        let msg = break_if_over!(receiver);
        if shares.len() == enough {
            continue;
        }
        match msg {
            NodeMessage::BingoCol(share) => {
                let r = node.lock().await.dom().get_root_of_unity(i);
                if comm.kzg_verify(share.uindex(), &r, share.first_share(), share.proof()) {
                    shares.push(share);
                    if shares.len() == enough {
                        let mut node = node.lock().await;
                        shares.sort();
                        let shares = kzg_interpolate_all(node.config(), &mut shares);
                        let is_byz = node.is_byz();
                        for (i, mut share) in shares.into_iter().enumerate() {
                            share.set_index(node.index());
                            if is_byz {
                                share.corrupt();
                            }

                            let bytes = enc!(Bingo, BingoCommand::NewRow, share);
                            node.contact(i, Arc::new(bytes))
                        }
                        node.send_message(NodeMessage::BingoBroadcastDoneRequest(Vec::new()))
                            .await;
                    }
                }
            }
            _ => panic!("A col message was expected"),
        }
    }
}

pub async fn line_manager(node: Wrapped<Node>) {
    let mut receiver = node.lock().await.subscribe(NodeMessage::BingoRowConst);
    let (enough, n, i) = {
        let node = node.lock().await;
        (2 * node.config().t() as usize + 1, node.n(), node.uindex())
    };
    let mut shares = Vec::with_capacity(enough);
    let comm = node.lock().await.get_comm().clone();
    loop {
        let msg = break_if_over!(receiver);
        if node.lock().await.set().len() == n {
            continue;
        }
        match msg {
            NodeMessage::BingoRow(share) => {
                let r = node.lock().await.dom().get_root_of_unity(share.uindex());
                assert!(comm.kzg_verify(i, &r, share.only_share().get(0), share.proof()));
                shares.push(share);
                if shares.len() == enough {
                    let node = node.lock().await;
                    shares.sort();
                    let shares = kzg_interpolate_all_without_proofs(node.config(), &shares);
                    node.send_message(NodeMessage::BingoBroadcastDoneRequest(shares))
                        .await;
                }
            }
            _ => panic!("A done message was expected"),
        }
    }
}

pub async fn done_manager(node: Wrapped<Node>) {
    let channels = vec![
        NodeMessage::BingoDoneConst,
        NodeMessage::BingoBroadcastDoneRequestConst,
    ];
    let mut receiver = node.lock().await.subscribe_multiple(&channels);
    let mut done_count = 0;
    let is_byz = node.lock().await.is_byz();
    let mut has_output = false;
    let mut has_broadcast = false;
    let enough = node.lock().await.t() * 2 + 1;
    loop {
        let msg = break_if_over!(receiver);
        if has_output {
            continue;
        }
        match msg {
            NodeMessage::BingoDone => {
                done_count += 1;
                if done_count >= enough && has_broadcast && !has_output {
                    Node::output(node.clone());
                    has_output = true;
                }
            }

            NodeMessage::BingoBroadcastDoneRequest(shares) => {
                if !has_broadcast {
                    has_broadcast = true;

                    node.lock().await.save_shares(shares).await;
                    if !is_byz {
                        let msg = init_message(NameSpace::Bingo, BingoCommand::NewDone);
                        node.lock().await.broadcast(msg, true).await;
                    }
                    if done_count >= enough {
                        Node::output(node.clone());
                        has_output = true;
                    }
                }
            }
            _ => panic!("A done message was expected"),
        }
    }
}
