use crate::crypto::crypto_lib::crypto_blstrs::polynomial::BlstrsPolynomial;
use anyhow::{ensure, Result};
use blstrs::{G1Projective, Scalar};
use group::Group;
use serde::{Deserialize, Serialize};
use std::ops::Mul;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlstrsFeldman {
    generator: G1Projective,
    max_degree: usize,
}

impl BlstrsFeldman {
    pub fn new(max_degree: usize, parameters: G1Projective) -> Self {
        Self {
            max_degree,
            generator: parameters,
        }
    }
}

// type Field = Scalar;
// type Group = G1Projective;
// type Polynomial = BlstrsPolynomial;
type Commitment = Vec<G1Projective>;
type Witness = ();

impl BlstrsFeldman {
    fn commit(&self, poly: &BlstrsPolynomial) -> Result<Vec<G1Projective>> {
        ensure!(
            poly.degree() <= self.max_degree,
            "Polynomial degree is too large!"
        );
        let mut commitment = Vec::with_capacity(poly.degree());
        for coeff in poly.iter() {
            commitment.push(self.generator.mul(coeff));
        }
        Ok(commitment)
    }

    fn open(&self, poly: &BlstrsPolynomial, x: &Scalar) -> Result<(Scalar, Witness)> {
        ensure!(
            poly.degree() <= self.max_degree,
            "Polynomial degree is too large!"
        );
        let eval = poly.eval(x);
        Ok((eval, ()))
    }

    fn open_commit(&self, poly: &BlstrsPolynomial, x: &Scalar) -> Result<(G1Projective, Witness)> {
        let (y, witness) = self.open(poly, x)?;
        Ok((self.generator.mul(y), witness))
    }

    fn verify(
        &self,
        commitment: &Commitment,
        x: &Scalar,
        value: &Scalar,
        witness: &Witness,
    ) -> bool {
        self.verify_from_commitment(commitment, x, &self.generator.mul(value), witness)
    }

    #[allow(unused_variables)]
    // TODO this isn't really used in the code but this is slow and should be changed to multi exp.
    fn verify_from_commitment(
        &self,
        commitment: &Commitment,
        x: &Scalar,
        value: &G1Projective,
        witness: &Witness,
    ) -> bool {
        if commitment.len() == 0 {
            return value == &G1Projective::identity();
        }

        let mut x_pows = x.clone();
        let mut sum = commitment[0].clone();
        for i in 1..commitment.len() {
            sum += commitment[i].mul(&x_pows);
            x_pows *= x;
        }
        value == &sum
    }
}
