use super::{
    crypto_lib::{
        evaluation_domain::BatchEvaluationDomain,
        fft::fft,
        lagrange::{lagrange_coefficients, lagrange_coefficients_at_zero},
        vss::{
            keys::InputSecret,
            ni_vss::dealing::{create_dealing, verify_dealing},
        },
    },
    data_structures::{
        commitment::Commitment, encryption::Encryption, keypair::PublicKey, share::Share,
    },
    Secret, Sign,
};
use crate::node::configuration::Configuration;
use aptos_crypto::Signature;
use blstrs::{G1Projective, Scalar};
use ff::Field;
use group::Group;
use rand::thread_rng;
use std::{collections::HashMap, ops::Mul};

pub fn is_valid_sign(p_key: &PublicKey, sign: &Sign, root: &[u8]) -> bool {
    sign.verify_arbitrary_msg(root, p_key.s_key()).is_ok()
}

pub async fn encode_shares(
    config: &Configuration,
    comm: &Commitment,
    ekeys: &[PublicKey],
    shares: &[Share],
) -> Vec<Encryption> {
    if shares.is_empty() {
        return Vec::new();
    }
    let mut selected_keys = Vec::new();
    shares
        .iter()
        .for_each(|s| selected_keys.push(ekeys[s.index() as usize].c_key()));
    let mut tasks = Vec::new();

    for i in 0..config.batch_size() {
        let comm = comm.clone();
        let shares = shares.to_vec();
        let selected_keys = selected_keys.clone();

        tasks.push(tokio::spawn(async move {
            let mut selected_coms = Vec::new();
            let randomness = shares
                .iter()
                .map(|share| *share.rand().get(i))
                .collect::<Vec<Scalar>>();
            let shares = shares
                .iter()
                .map(|share| {
                    selected_coms.push(*comm.get(i, share.index() as usize));
                    *share.only_share().get(i)
                })
                .collect::<Vec<Scalar>>();
            create_dealing(
                &comm.base()[1],
                &selected_coms,
                &selected_keys,
                &shares,
                &randomness,
            )
        }));
    }

    let mut res = Vec::new();
    for task in tasks {
        res.push(task.await.unwrap());
    }
    res
}

pub fn verify_encryption(
    config: &Configuration,
    encs: &[Encryption],
    missing: &[usize],
    missing_coms: &[Vec<G1Projective>],
    comm: &Commitment,
    ekeys: &[PublicKey],
) -> bool {
    let mut selected_keys = Vec::new();
    missing.iter().for_each(|i| {
        selected_keys.push(ekeys[*i].c_key());
    });
    !(0..config.batch_size()).any(|i| {
        let mut selected_coms: Vec<G1Projective> = Vec::new();
        missing_coms.iter().for_each(|p| {
            selected_coms.push(p[i]);
        });
        !verify_dealing(&comm.base()[1], &selected_coms, &selected_keys, &encs[i])
    })
}

pub fn interpolate(
    sc: &Configuration,
    shares: &HashMap<u16, Share>,
    secrets: &Option<Vec<Secret>>,
) -> bool {
    let selected = shares.keys().map(|i| *i as usize).collect::<Vec<_>>();
    let lagr = lagrange_coefficients_at_zero(sc.get_batch_evaluation_domain(), &selected);
    !(0..sc.batch_size()).any(|b_index| {
        let mut s = Scalar::zero();
        let mut r = Scalar::zero();

        for (i, j) in selected.iter().enumerate() {
            s += lagr[i] * shares[&(*j as u16)].only_share().get(b_index);
            r += lagr[i] * shares[&(*j as u16)].rand().get(b_index);
        }
        if let Some(secrets) = secrets {
            secrets[b_index] != s
        } else {
            false
        }
    })
}

pub fn interpolate_on_zero(sc: &Configuration, shares: &HashMap<u16, Share>) -> Scalar {
    let selected = shares.keys().map(|i| *i as usize).collect::<Vec<_>>();
    let lagr = lagrange_coefficients_at_zero(sc.get_batch_evaluation_domain(), &selected);

    let mut s = Scalar::zero();

    for (i, j) in selected.iter().enumerate() {
        s += lagr[i] * shares[&(*j as u16)].only_share().get(0);
    }
    s
}

pub fn interpolate_on_single(
    evals: &[Scalar],
    selected: &[usize],
    alphas: &[Scalar],
    batch_dom: &BatchEvaluationDomain,
) -> Vec<Scalar> {
    let lagr = lagrange_coefficients(batch_dom, selected, alphas);

    lagr.iter()
        .map(|lagr| {
            let mut s = Scalar::zero();
            for i in 0..selected.len() {
                s += lagr[i].mul(evals[i]);
            }
            s
        })
        .collect()
}

pub fn interpolate_on_single_with_proofs(
    evals: &[Scalar],
    selected: &[usize],
    alphas: &[Scalar],
    proofs: &[G1Projective],
    batch_dom: &BatchEvaluationDomain,
) -> Vec<(Scalar, G1Projective)> {
    let lagr = lagrange_coefficients(batch_dom, selected, alphas);

    lagr.iter()
        .map(|lagr| {
            let mut s = Scalar::zero();
            for i in 0..selected.len() {
                s += lagr[i].mul(evals[i]);
            }
            let mut p = G1Projective::identity();
            for i in 0..selected.len() {
                p += proofs[i].mul(lagr[i]);
            }
            (s, p)
        })
        .collect()
}

pub fn interpolate_on(
    evals: &[Scalar],
    rands: &[Scalar],
    selected: &[usize],
    alphas: &[Scalar],
    batch_dom: &BatchEvaluationDomain,
) -> (Vec<Scalar>, Vec<Scalar>) {
    let lagr = lagrange_coefficients(batch_dom, selected, alphas);
    let mut rands_res = Vec::with_capacity(alphas.len() + rands.len());
    (
        lagr.iter()
            .map(|lagr| {
                let mut s = Scalar::zero();
                let mut r = Scalar::zero();
                for i in 0..selected.len() {
                    s += lagr[i].mul(evals[i]);
                    r += lagr[i].mul(rands[i]);
                }
                rands_res.push(r);
                s
            })
            .collect(),
        rands_res,
    )
}

pub fn complete_evaluations(
    n: usize,
    shares: &[Share],
    batch_dom: &BatchEvaluationDomain,
) -> Vec<(Scalar, Option<G1Projective>)> {
    let (mut selected, mut evals) = (Vec::new(), Vec::new());
    for s in shares {
        selected.push(s.uindex());
        evals.push(*s.first_share());
    }
    let missing = (0..n)
        .filter(|i| !selected.contains(i))
        .map(|i| batch_dom.get_root_of_unity(i))
        .collect::<Vec<_>>();
    let mut interpolated_shares = interpolate_on_single(&evals, &selected, &missing, batch_dom);

    let mut i = 0;
    let mut res = Vec::with_capacity(n);
    for share in shares.iter() {
        while share.index() > i {
            res.push((interpolated_shares.remove(0), None));
            i += 1;
        }
        res.push((*share.first_share(), Some(*share.proof())));
        i += 1;
    }
    while i < n as u16 {
        res.push((interpolated_shares.remove(0), None));
        i += 1;
    }
    res
}

pub fn complete_evaluations_with_proofs(
    n: usize,
    shares: &[Share],
    batch_dom: &BatchEvaluationDomain,
) -> Vec<(Scalar, G1Projective)> {
    let (mut selected, mut evals, mut proofs) = (Vec::new(), Vec::new(), Vec::new());
    for s in shares {
        selected.push(s.uindex());
        evals.push(*s.first_share());
        proofs.push(*s.proof())
    }
    let missing = (0..n)
        .filter(|i| !selected.contains(i))
        .map(|i| batch_dom.get_root_of_unity(i))
        .collect::<Vec<_>>();
    let mut interpolated_shares =
        interpolate_on_single_with_proofs(&evals, &selected, &missing, &proofs, batch_dom);

    let mut i = 0;
    let mut res = Vec::with_capacity(n);
    for share in shares.iter() {
        while share.index() > i {
            res.push(interpolated_shares.remove(0));
            i += 1;
        }
        res.push((*share.first_share(), *share.proof()));
        i += 1;
    }
    while i < n as u16 {
        res.push(interpolated_shares.remove(0));
        i += 1;
    }
    res
}

pub fn kzg_interpolate_all(sc: &Configuration, shares: &mut [Share]) -> Vec<Share> {
    for s in shares.iter_mut() {
        s.set_index(s.index() + 1)
    }
    let n = sc.get_evaluation_domain().N;
    let res = complete_evaluations_with_proofs(n, shares, sc.get_batch_evaluation_domain());

    res.into_iter()
        .skip(1)
        .take(sc.n() as usize)
        .enumerate()
        .map(|(i, (s, w))| Share::kzg_new(i as u16, s, w))
        .collect()
}

pub fn kzg_interpolate_all_without_proofs(sc: &Configuration, shares: &[Share]) -> Vec<Share> {
    let n = sc.get_evaluation_domain().N;
    let res = complete_evaluations(n, shares, sc.get_batch_evaluation_domain());

    res.into_iter()
        .take(sc.n() as usize)
        .enumerate()
        .map(|(i, (s, _))| Share::kzg_new_without_proof(i as u16, s))
        .collect()
}

pub fn kzg_interpolate_one(
    evals: &[Scalar],
    selected: &[usize],
    alpha: Scalar,
    batch_dom: &BatchEvaluationDomain,
) -> Scalar {
    let lagr = &lagrange_coefficients(batch_dom, selected, &[alpha])[0];
    let mut s = Scalar::zero();
    for i in 0..selected.len() {
        s += lagr[i].mul(evals[i]);
    }
    s
}

pub fn interpolate_one(
    evals: &[Scalar],
    rands: &[Scalar],
    selected: &[usize],
    alpha: Scalar,
    batch_dom: &BatchEvaluationDomain,
) -> (Scalar, Scalar) {
    let lagr = &lagrange_coefficients(batch_dom, selected, &[alpha])[0];
    let mut s = Scalar::zero();
    let mut r = Scalar::zero();
    for i in 0..selected.len() {
        r += lagr[i].mul(rands[i]);
        s += lagr[i].mul(evals[i]);
    }
    (s, r)
}

pub fn kzg_interpolate_specific_share(
    batch_dom: &BatchEvaluationDomain,
    shares: &[Share],
    i: usize,
) -> Share {
    let selected = &shares
        .iter()
        .map(|s| s.index() as usize)
        .collect::<Vec<_>>();
    let r = batch_dom.get_root_of_unity(i);
    let b = shares[0].only_share().len();
    let mut s = Vec::with_capacity(b);
    for i in 0..b {
        let mut s_evals = Vec::new();
        for s in shares {
            s_evals.push(*s.only_share().get(i));
        }
        let i_s = kzg_interpolate_one(&s_evals, selected, r, batch_dom);
        s.push(i_s);
    }
    Share::new(i as u16, s, Vec::new())
}

pub fn interpolate_specific_share(
    batch_dom: &BatchEvaluationDomain,
    shares: &[Share],
    i: usize,
) -> Share {
    let selected = &shares
        .iter()
        .map(|s| s.index() as usize)
        .collect::<Vec<_>>();
    let r = batch_dom.get_root_of_unity(i);
    let b = shares[0].only_share().len();
    let mut s = Vec::with_capacity(b);
    let mut g = Vec::with_capacity(b);
    for i in 0..b {
        let (mut s_evals, mut r_evals) = (Vec::new(), Vec::new());
        for s in shares {
            s_evals.push(*s.only_share().get(i));
            r_evals.push(*s.rand().get(i));
        }
        let (i_s, i_r) = interpolate_one(&s_evals, &r_evals, selected, r, batch_dom);
        s.push(i_s);
        g.push(i_r);
    }
    Share::new(i as u16, s, g)
}

pub fn compute_comm_and_shares(sc: &Configuration) -> (Commitment, Vec<Share>, Vec<Secret>) {
    let n = sc.n() as usize;
    let b = sc.batch_size();
    let mut secrets = Vec::new();
    let mut shares: Vec<Vec<Scalar>> = vec![Vec::with_capacity(b); n];
    let mut rands: Vec<Vec<Scalar>> = vec![Vec::with_capacity(b); n];
    let mut comm = Commitment::new(*sc.base());
    for _ in 0..b {
        let mut rng = thread_rng();
        let s = InputSecret::new_random(sc.get_threshold(), true, &mut rng);
        secrets.push(s.get_secret_a());
        let f = s.get_secret_f();
        let r = s.get_secret_r();

        let mut f_evals = fft(f, sc.get_evaluation_domain());
        f_evals.truncate(n);

        let mut r_evals = fft(r, sc.get_evaluation_domain());
        r_evals.truncate(n);

        comm.add(&f_evals, &r_evals);
        for (i, (s, r)) in shares.iter_mut().zip(rands.iter_mut()).enumerate() {
            s.push(f_evals[i]);
            r.push(r_evals[i]);
        }
    }
    (
        comm,
        shares
            .into_iter()
            .zip(rands)
            .enumerate()
            .map(|(i, (s, r))| Share::new(i as u16, s, r))
            .collect(),
        secrets,
    )
}
