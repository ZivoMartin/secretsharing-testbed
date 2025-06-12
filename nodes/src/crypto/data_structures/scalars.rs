use blstrs::Scalar;
use ff::Field;
use serde::{Deserialize, Serialize};
use std::{
    cmp::{Eq, PartialEq},
    ops::Neg,
};

pub static SIZE: usize = 2;
pub const S: u32 = 32;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchScalar {
    s: Vec<Scalar>,
}

impl BatchScalar {
    pub fn new(s: Vec<Scalar>) -> Self {
        BatchScalar { s }
    }

    pub fn new_simpl(s: Scalar) -> Self {
        BatchScalar { s: vec![s] }
    }

    pub fn empty() -> Self {
        Self { s: Vec::new() }
    }

    pub fn zero(b: usize) -> Self {
        Self::new(vec![Scalar::zero(); b])
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.batch().len()
    }

    pub fn batch_mut(&mut self) -> &mut Vec<Scalar> {
        &mut self.s
    }

    pub fn batch(&self) -> &Vec<Scalar> {
        &self.s
    }

    pub fn get(&self, i: usize) -> &Scalar {
        &self.s[i]
    }
}

macro_rules! from {
    () => {};

    ($the_type:ty, $($rest:ty),*) => {
        from!($the_type);
        from!($($rest),*);
    };

    ($the_type:ty) => {
        impl From<$the_type> for BatchScalar {
            fn from(value: $the_type) -> Self {
                BatchScalar {
                    s: vec!(Scalar::from(value as u64)),
                }
            }
        }
    };
}

from!(u64, i64, u32, i32, u128, i128, u8, i8);

impl Neg for BatchScalar {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.s.iter_mut().for_each(|s| *s = -*s);
        self
    }
}

impl Eq for BatchScalar {}

impl PartialEq for BatchScalar {
    fn eq(&self, other: &Self) -> bool {
        self.s == other.s
    }
}
