use crate::crypto::crypto_lib::{crypto_blstrs::polynomial::BlstrsPolynomial, random_scalar};

use anyhow::{ensure, Result};
use blstrs::{Bls12, G1Projective, G2Projective, Scalar};
use ff::Field;
use group::{Curve, Group};
use pairing::Engine;
use serde::{Deserialize, Serialize};
use std::ops::{Mul, Sub};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BlstrsKZG {
    powers_of_tau: Vec<G1Projective>,
    g2_tau: G2Projective,
    generators: (G1Projective, G2Projective),
}

impl BlstrsKZG {
    pub fn new<R>(rng: &mut R, max_degree: usize, generators: (G1Projective, G2Projective)) -> Self
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        let tau = random_scalar(rng);
        let g2_tau = generators.1.mul(&tau);
        let powers_of_tau = powers_of_tau(max_degree, &generators.0, &tau);
        Self {
            generators,
            powers_of_tau,
            g2_tau,
        }
    }
}

pub type Polynomial = BlstrsPolynomial;
pub type Commitment = G1Projective;
pub type Witness = G1Projective;

impl BlstrsKZG {
    pub fn commit(&self, poly: &Polynomial) -> Result<Commitment> {
        ensure!(
            poly.degree().overflowing_add(1).0 <= self.powers_of_tau.len(),
            "Polynomial degree too large!"
        );
        Ok(eval_poly_at_tau(&self.powers_of_tau, poly))
    }

    pub fn open(&self, p: &Polynomial, eval: Scalar, x: &Scalar) -> Result<Witness> {
        let divisor = Polynomial::from(vec![-*x, Scalar::one()]);
        let dividend = p.clone() + Polynomial::new(vec![-eval]);
        let witness = dividend.div_ref(&divisor).unwrap();
        Ok(eval_poly_at_tau(&self.powers_of_tau, &witness))
    }

    pub fn batch_open<R>(
        &self,
        rng: &mut R,
        p: &Polynomial,
        evals: &[Scalar],
        points: &[Scalar],
    ) -> Result<(Witness, Vec<(Scalar, Witness)>)>
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        assert_eq!(
            evals.len(),
            points.len(),
            "Mismatch in evals and points length"
        );

        let r = random_scalar(rng);

        let z = points
            .iter()
            .fold(Polynomial::from(vec![Scalar::one()]), |acc, &xi| {
                acc.mul(&Polynomial::from(vec![-xi, Scalar::one()]))
            });

        let mut p_batch = p.clone();
        let mut r_i = Scalar::one();
        let mut individual_witnesses = Vec::new();

        for (xi, vi) in points.iter().zip(evals.iter()) {
            let li = Polynomial::from(vec![-*xi, Scalar::one()]);
            let li_eval = li.mul(&Polynomial::new(vec![*vi * r_i]));
            p_batch = p_batch.sub(&li_eval);

            let witness_poly = p.sub(&li_eval).div_ref(&li).unwrap();
            let witness = eval_poly_at_tau(&self.powers_of_tau, &witness_poly);
            individual_witnesses.push((*xi, witness));

            r_i *= r;
        }

        let witness = p_batch.div_ref(&z).unwrap();
        let batch_proof = eval_poly_at_tau(&self.powers_of_tau, &witness);

        Ok((batch_proof, individual_witnesses))
    }

    pub fn verify_single_evaluation(
        &self,
        commitment: &Commitment,
        x: &Scalar,
        value: &Scalar,
        batch_witness: &Witness,
    ) -> bool {
        // Compute the left-hand side: e(C - value * G, G2)
        let lhs = Bls12::pairing(
            &commitment.sub(&self.generators.0.mul(value)).to_affine(),
            &self.generators.1.to_affine(),
        );

        // Compute the right-hand side: e(W, x * tau * G2)
        let rhs = Bls12::pairing(
            &batch_witness.to_affine(),
            &(self.generators.1.mul(x)).to_affine(),
        );

        lhs == rhs
    }

    pub fn verify(
        &self,
        commitment: &Commitment,
        x: &Scalar,
        value: &Scalar,
        witness: &Witness,
    ) -> bool {
        self.verify_from_commitment(commitment, x, &self.generators.0.mul(value), witness)
    }

    pub fn verify_from_commitment(
        &self,
        commitment: &Commitment,
        x: &Scalar,
        value: &G1Projective,
        witness: &Witness,
    ) -> bool {
        Bls12::pairing(
            &commitment.sub(value).to_affine(),
            &self.generators.1.to_affine(),
        ) == Bls12::pairing(
            &witness.to_affine(),
            &self.g2_tau.sub(&self.generators.1.mul(x)).to_affine(),
        ) // SED REPLACE 1A
    }
}

pub fn powers_of_tau(
    max_degree: usize,
    generator: &G1Projective,
    tau: &Scalar,
) -> Vec<G1Projective> {
    let mut powers_of_tau = Vec::with_capacity(max_degree + 1);
    let mut exp = Scalar::one();
    for _ in 0..=max_degree {
        powers_of_tau.push(generator.mul(&exp));
        exp *= tau;
    }
    powers_of_tau
}

pub fn eval_poly_at_tau(powers_of_tau: &[G1Projective], poly: &BlstrsPolynomial) -> G1Projective {
    powers_of_tau
        .iter()
        .zip(poly.iter())
        .map(|(x, y)| x.mul(y))
        .fold(G1Projective::identity(), std::ops::Add::add)
}
