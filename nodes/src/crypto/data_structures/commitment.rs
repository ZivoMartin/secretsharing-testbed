use super::bivariate_polynomial::Polynomial;
use super::{share::Share, Base};
use crate::crypto::crypto_lib::{
    crypto_blstrs::poly_commit::kzg::BlstrsKZG, evaluation_domain::EvaluationDomain, fft::fft,
    vss::common::random_scalars_range,
};
use blstrs::{G1Projective, G2Projective, Scalar};
use ff::Field;
use group::Group;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Debug, Clone, Deserialize, Eq, PartialEq)]
pub struct Commitment {
    kzg_setup: Option<Arc<BlstrsKZG>>,
    base: Option<Base>,
    comms: Vec<Vec<G1Projective>>,
}

impl Commitment {
    pub fn new(base: Base) -> Self {
        Self {
            base: Some(base),
            comms: Vec::new(),
            kzg_setup: None,
        }
    }

    pub fn new_kzg<R>(rng: &mut R, degree: usize) -> Self
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        Self {
            base: None,
            comms: vec![Vec::new()],
            kzg_setup: Some(Arc::new(BlstrsKZG::new(
                rng,
                degree,
                (G1Projective::generator(), G2Projective::generator()),
            ))),
        }
    }

    pub fn kzg_add(&mut self, poly: &Polynomial) {
        if self.comms.is_empty() {
            self.comms.push(Vec::new());
        }
        self.comms[0].push(BlstrsKZG::commit(self.kzg_setup.as_ref().unwrap(), poly).unwrap());
    }

    pub fn kzg_push(&mut self, commit: G1Projective) {
        if self.comms.is_empty() {
            self.comms.push(Vec::new());
        }
        self.comms[0].push(commit);
    }

    pub fn kzg_verify_poly(&self, poly: &Polynomial, i: usize) -> bool {
        self.comms[0][i] == BlstrsKZG::commit(self.kzg_setup.as_ref().unwrap(), poly).unwrap()
    }

    pub fn kzg_setup(&self) -> Arc<BlstrsKZG> {
        self.kzg_setup.as_ref().unwrap().clone()
    }

    fn compute_line(&self, shares: &[Scalar], rand: &[Scalar]) -> Vec<G1Projective> {
        let mut comm = Vec::new();
        let b = self.base.as_ref().unwrap();
        for (s, r) in shares.iter().zip(rand.iter()) {
            comm.push(G1Projective::multi_exp(b, &[*s, *r]));
        }
        comm
    }

    pub fn add(&mut self, shares: &[Scalar], rand: &[Scalar]) {
        self.comms.push(self.compute_line(shares, rand));
    }

    pub fn add_computed(
        &mut self,
        p: &[Scalar],
        rand: &[Scalar],
        domain: &EvaluationDomain,
        n: usize,
    ) -> (Vec<Scalar>, Vec<Scalar>) {
        let mut line = fft(p, domain);
        let mut line_rand = fft(rand, domain);
        line.truncate(n);
        line_rand.truncate(n);
        self.comms.push(self.compute_line(&line, &line_rand));
        (line, line_rand)
    }

    pub fn base(&self) -> &Base {
        self.base.as_ref().unwrap()
    }

    pub fn batch_size(&self) -> usize {
        self.comms.len()
    }

    pub fn all(&self) -> &Vec<Vec<G1Projective>> {
        &self.comms
    }

    pub fn get(&self, b: usize, i: usize) -> &G1Projective {
        &self.comms[b][i]
    }

    pub fn get_col(&self, i: usize) -> Vec<G1Projective> {
        (0..self.batch_size()).map(|b| *self.get(b, i)).collect()
    }

    pub fn verify_line(&self, i: usize, shares: &[Scalar], rand: &[Scalar]) -> bool {
        self.comms[i][..shares.len()] == self.compute_line(shares, rand)
    }

    pub fn kzg_verify(&self, i: usize, j: &Scalar, share: &Scalar, proof: &G1Projective) -> bool {
        self.kzg_setup().verify(self.get(0, i), j, share, proof)
    }

    pub fn verify_on(&self, b: usize, i: usize, share: &[Scalar; 2]) -> bool {
        let e_com = G1Projective::multi_exp(self.base.as_ref().unwrap(), share);
        self.get(b, i).eq(&e_com)
    }

    pub fn verify(&self, share: &Share) -> bool {
        let i = share.index() as usize;
        !share
            .only_share()
            .batch()
            .iter()
            .zip(share.rand().batch())
            .enumerate()
            .any(|(b, (s, r))| !self.verify_on(b, i, &[*s, *r]))
    }

    pub fn batch_verify(&self, shares: &[Share]) -> bool {
        if shares.is_empty() {
            return true;
        }
        let lambdas = {
            let mut rng = thread_rng();
            random_scalars_range(&mut rng, u64::MAX, shares.len())
        };
        !(0..self.batch_size()).any(|i| {
            let mut s = Scalar::zero();
            let mut r = Scalar::zero();
            for (lambda, share) in lambdas.iter().zip(shares.iter()) {
                s += lambda * share.only_share().get(i);
                r += lambda * share.rand().get(i);
            }

            let mut coms = Vec::with_capacity(shares.len());
            for share in shares.iter() {
                coms.push(*self.get(i, share.index() as usize))
            }
            let com_pos: G1Projective =
                G1Projective::multi_exp(self.base.as_ref().unwrap(), [s, r].as_slice());
            let com = G1Projective::multi_exp(&coms, &lambdas);
            com_pos != com
        })
    }
}
