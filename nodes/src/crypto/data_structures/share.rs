use super::scalars::BatchScalar;
use blstrs::G1Projective;
use blstrs::Scalar;
use ff::Field;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ops::AddAssign;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Share {
    shares: [BatchScalar; 2],
    proof: Option<G1Projective>,
    index: u16,
}

impl Share {
    pub fn kzg_new_without_proof(index: u16, share: Scalar) -> Self {
        Self {
            shares: [BatchScalar::new_simpl(share), BatchScalar::empty()],
            proof: None,
            index,
        }
    }

    pub fn kzg_new(index: u16, share: Scalar, proof: G1Projective) -> Self {
        Self {
            shares: [BatchScalar::new_simpl(share), BatchScalar::empty()],
            proof: Some(proof),
            index,
        }
    }

    pub fn new_simpl(index: u16, share: [Scalar; 2]) -> Self {
        Share {
            index,
            proof: None,
            shares: [
                BatchScalar::new_simpl(share[0]),
                BatchScalar::new_simpl(share[1]),
            ],
        }
    }

    pub fn new(i: u16, share: Vec<Scalar>, rands: Vec<Scalar>) -> Self {
        Share {
            proof: None,
            index: i,
            shares: [BatchScalar::new(share), BatchScalar::new(rands)],
        }
    }

    pub fn empty(b: usize) -> Self {
        Self {
            proof: None,
            index: 0,
            shares: [BatchScalar::zero(b), BatchScalar::zero(b)],
        }
    }

    pub fn proof(&self) -> &G1Projective {
        self.proof.as_ref().unwrap()
    }

    pub fn set_index(&mut self, index: u16) {
        self.index = index;
    }

    pub fn first_rand(&self) -> &Scalar {
        self.rand().get(0)
    }

    pub fn first_share(&self) -> &Scalar {
        self.only_share().get(0)
    }

    pub fn share(&self) -> &[BatchScalar; 2] {
        &self.shares
    }

    pub fn rand(&self) -> &BatchScalar {
        &self.shares[1]
    }

    pub fn only_share(&self) -> &BatchScalar {
        &self.shares[0]
    }

    pub fn uindex(&self) -> usize {
        self.index as usize
    }

    pub fn index(&self) -> u16 {
        self.index
    }

    pub fn get(&self, i: usize) -> [Scalar; 2] {
        [*self.only_share().get(i), *self.rand().get(i)]
    }

    pub fn corrupt(&mut self) {
        for s in self.shares[0].batch_mut() {
            s.add_assign(Scalar::one());
        }

        for r in self.shares[1].batch_mut() {
            r.add_assign(Scalar::one());
        }
    }
}

impl PartialEq for Share {
    fn eq(&self, other: &Share) -> bool {
        self.index() == other.index()
    }
}

impl PartialOrd for Share {
    fn partial_cmp(&self, other: &Share) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Share {}

impl Ord for Share {
    fn cmp(&self, other: &Share) -> Ordering {
        self.index().cmp(&other.index())
    }
}
