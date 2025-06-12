use super::crypto::SendMessage;
use crate::{
    crypto::{Commitment, Polynomial, Share},
    node::node::Node,
};
use futures::stream::{self, StreamExt};
use global_lib::{enc, messages::HavenCommand, Wrapped};
use rand::thread_rng;

pub async fn deal(node: Wrapped<Node>) {
    let mut node = node.lock().await;
    let config = node.config();
    let dom = config.get_evaluation_domain();
    let n = config.n() as usize;
    let mut main_comm = Commitment::new(*config.base());
    let mut comms = vec![Commitment::new(*config.base()); n];
    let mut evals = vec![Vec::new(); n];
    {
        let rng = &mut thread_rng();

        for _ in 0..config.batch_size() {
            let recovery = Polynomial::random(None, config.l() as usize, rng);
            let random_recovery = Polynomial::random(None, config.l() as usize, rng);
            main_comm.add(recovery.fields(), random_recovery.fields());
            let fft_evals = recovery.fft(dom, config.n() as usize);
            let share_polynomials = fft_evals
                .into_iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        Polynomial::random_such_that(rng, config.t() as usize, i, e),
                        Polynomial::random(None, config.t() as usize, rng),
                    )
                })
                .collect::<Vec<_>>();
            let this_evals = share_polynomials
                .iter()
                .map(|(p, r)| (p.fft(dom, n), r.fft(dom, n)))
                .collect::<Vec<_>>();
            for (comm, (p, r)) in comms.iter_mut().zip(this_evals.iter()) {
                comm.add(p, r);
            }
            (0..config.n() as usize).for_each(|i| {
                evals[i].push(
                    this_evals
                        .iter()
                        .map(|(p, r)| (p[i], r[i]))
                        .collect::<Vec<_>>(),
                )
            });
        }
    }
    let root: Vec<u8> = Vec::new();
    let messages = stream::iter(evals.into_iter().enumerate())
        .map(|(i, evals)| {
            let root = root.clone();
            let comms = comms.clone();
            let main_comm = main_comm.clone();
            tokio::spawn(async move {
                (
                    i,
                    enc!(
                        Haven,
                        HavenCommand::Send,
                        SendMessage {
                            root: root,
                            comms: (comms, main_comm),
                            evals: evals
                                .into_iter()
                                .map(|eval| {
                                    let (s, r): (Vec<_>, Vec<_>) = eval.into_iter().unzip();
                                    Share::new(i as u16, s, r)
                                })
                                .collect()
                        }
                    ),
                )
            })
        })
        .buffer_unordered(10)
        .map(|res| res.unwrap())
        .collect::<Vec<_>>()
        .await;
    let cor = node.dealer_corruption();
    for (i, message) in messages.into_iter().skip(cor as usize) {
        node.give_contact(i, message)
    }
}
