use super::{eval, interpolate};
use crate::crypto::crypto_lib::{evaluation_domain::EvaluationDomain, fft::fft, random_scalar};
use anyhow::{ensure, Result};
use blstrs::Scalar;
use ff::Field;
pub use num_traits::Zero;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};
use std::ops::{Add, Mul};
use std::vec;

use crate::crypto::crypto_lib::random_scalars;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlstrsPolynomial {
    coeffs: Vec<Scalar>,
}

impl BlstrsPolynomial {
    pub fn new(mut coeffs: Vec<Scalar>) -> Self {
        while Some(&Scalar::zero()) == coeffs.last() {
            coeffs.pop();
        }
        Self { coeffs }
    }

    pub fn fft(&self, dom: &EvaluationDomain, n: usize) -> Vec<Scalar> {
        fft(&self.coeffs, dom).drain(0..n).collect()
    }

    pub fn random<R>(first_coeff: Option<Scalar>, d: usize, rng: &mut R) -> Self
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        let mut coeffs = random_scalars(d + 1, rng);
        if let Some(first) = first_coeff {
            coeffs[0] = first;
        }
        Self { coeffs }
    }

    pub fn random_such_that<R>(rng: &mut R, d: usize, index: usize, e: Scalar) -> Self
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        let mut p = Self::random(None, d, rng);
        let to_adjust = p.eval(&Scalar::from(index as u64));
        p.coeffs[0] += e - to_adjust;
        p
    }

    pub fn from(coeffs: Vec<Scalar>) -> Self {
        Self { coeffs }
    }

    pub fn fields(&self) -> &Vec<Scalar> {
        &self.coeffs
    }

    pub fn eval(&self, x: &Scalar) -> Scalar {
        eval(&self.coeffs, x, Scalar::zero())
    }

    pub fn degree(&self) -> usize {
        self.coeffs.len().overflowing_sub(1).0
    }

    pub fn sample<R>(rng: &mut R, degree: usize, mut fixed_points: HashMap<usize, Scalar>) -> Self
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        let degree_plus_one = degree.overflowing_add(1).0;
        assert!(
            fixed_points.len() <= degree_plus_one,
            "More fixed points than degree!"
        );

        let mut ys = Vec::with_capacity(degree_plus_one);
        let mut xs = Vec::with_capacity(degree_plus_one);

        let mut i = 0;
        while fixed_points.len() < degree_plus_one {
            if let Entry::Vacant(e) = fixed_points.entry(i) {
                e.insert(random_scalar(rng));
            }

            if let Entry::Vacant(e) = fixed_points.entry(i) {
                e.insert(random_scalar(rng));
            }
            i += 1;
        }

        for (x, y) in fixed_points {
            xs.push(Scalar::from(x as u64));
            ys.push(y);
        }

        Self::new(interpolate(
            &xs,
            ys,
            |s| s.invert().unwrap(),
            Scalar::zero(),
        ))
    }

    pub fn div_ref(&self, rhs: &Self) -> Result<Self> {
        ensure!(!rhs.is_zero(), "Division by 0!");
        if self.is_zero() || self.degree() < rhs.degree() {
            return Ok(Self::zero());
        }
        let d = rhs.degree();
        let mut q = vec![Scalar::zero(); self.degree() - d + 1];
        let mut r = self.clone();
        let c = rhs.coeffs.last().unwrap().invert().unwrap();

        while !r.is_zero() && r.degree() >= d {
            let s_coeff = r.coeffs.last().unwrap().mul(&c);
            let s_index = r.degree() - d;
            r = Self::new(
                r.coeffs
                    .into_iter()
                    .enumerate()
                    .map(|(i, f)| match i {
                        x if x >= s_index => {
                            let offset = x - s_index;
                            f - s_coeff.mul(&rhs.coeffs[offset])
                        }
                        _ => f,
                    })
                    .collect(),
            );
            q[s_index] = s_coeff;
        }
        Ok(Self::new(q))
    }

    pub fn iter(&self) -> std::slice::Iter<Scalar> {
        self.coeffs.iter()
    }
}

impl From<Vec<Scalar>> for BlstrsPolynomial {
    fn from(coeffs: Vec<Scalar>) -> Self {
        Self::new(coeffs)
    }
}

impl From<BlstrsPolynomial> for Vec<Scalar> {
    fn from(val: BlstrsPolynomial) -> Self {
        val.coeffs
    }
}

impl Add<Self> for BlstrsPolynomial {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let self_coeffs: Vec<Scalar> = self.into();
        let rhs_coeffs: Vec<Scalar> = rhs.into();
        let (mut larger, smaller) = if self_coeffs.len() > rhs_coeffs.len() {
            (self_coeffs, rhs_coeffs)
        } else {
            (rhs_coeffs, self_coeffs)
        };
        for i in 0..smaller.len() {
            larger[i] += smaller[i];
        }
        Self::new(larger)
    }
}

impl Zero for BlstrsPolynomial {
    fn zero() -> Self {
        Self::new(vec![])
    }

    fn is_zero(&self) -> bool {
        self.degree() == usize::MAX
    }
}

impl BlstrsPolynomial {
    pub fn mul(&self, other: &BlstrsPolynomial) -> BlstrsPolynomial {
        let degree_a = self.coeffs.len();
        let degree_b = other.coeffs.len();

        // Resulting polynomial degree is (degree_a + degree_b - 2)
        let mut result_coeffs = vec![Scalar::zero(); degree_a + degree_b - 1];

        // Perform coefficient-wise multiplication
        for (i, &a_coeff) in self.coeffs.iter().enumerate() {
            for (j, &b_coeff) in other.coeffs.iter().enumerate() {
                result_coeffs[i + j] += a_coeff * b_coeff;
            }
        }

        BlstrsPolynomial {
            coeffs: result_coeffs,
        }
    }

    pub fn sub(&self, other: &BlstrsPolynomial) -> BlstrsPolynomial {
        let max_degree = usize::max(self.coeffs.len(), other.coeffs.len());
        let mut result_coeffs = vec![Scalar::zero(); max_degree];

        for i in 0..max_degree {
            let a_coeff = self.coeffs.get(i).cloned().unwrap_or(Scalar::zero());
            let b_coeff = other.coeffs.get(i).cloned().unwrap_or(Scalar::zero());
            result_coeffs[i] = a_coeff - b_coeff;
        }

        BlstrsPolynomial {
            coeffs: result_coeffs,
        }
    }
}

impl IntoIterator for BlstrsPolynomial {
    type Item = Scalar;
    type IntoIter = vec::IntoIter<Scalar>;

    fn into_iter(self) -> Self::IntoIter {
        self.coeffs.into_iter()
    }
}
