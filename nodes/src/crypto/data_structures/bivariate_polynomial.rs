use blstrs::Scalar;
use ff::Field;
use serde::{Deserialize, Serialize};

pub use crate::crypto::{
    crypto_lib::{
        crypto_blstrs::polynomial::BlstrsPolynomial as Polynomial,
        evaluation_domain::smallest_power_of_2_greater_or_eq_than,
        fft::{fft, ifft_assign},
        random_scalars,
    },
    scheme::interpolate_on_single,
};
use crate::node::configuration::Configuration;

#[derive(Serialize, Deserialize)]
pub struct BiVariatePoly {
    fields: Vec<Polynomial>,
}

impl BiVariatePoly {
    pub fn random<R>(
        _sc: &Configuration,
        _secrets: Option<Vec<Scalar>>,
        tx: usize,
        ty: usize,
        rng: &mut R,
    ) -> BiVariatePoly
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        // match secrets {
        //     Some(secrets) => {
        //         let batch_dom = sc.get_batch_evaluation_domain();
        //         let n = sc.n() as usize;
        //         let t = sc.t() as usize;
        //         assert!(secrets.len() <= t);
        //         let s = secrets[0];
        //         let mut evals = random_scalars(2 * t + 1, rng);
        //         for (i, s) in (t + 1..2 * t + 1).zip(secrets.into_iter()) {
        //             evals[i] = s
        //         }
        //         println!("Base evals: {evals:?}\n");
        //         let missing = (t + 1..=2 * t)
        //             .map(|i| batch_dom.get_root_of_unity(i))
        //             .collect::<Vec<_>>();
        //         let mut selected = (n..n + t).collect::<Vec<_>>();
        //         selected.append(&mut (0..=t).collect::<Vec<_>>());
        //         selected.sort();
        //         let mut interpolated =
        //             interpolate_on_single(&evals, &selected, &missing, batch_dom);
        //         let mut pol = evals[0..=t].to_vec();
        //         pol.append(&mut interpolated);
        //         println!("Before: {pol:?}\n");
        //         ifft_assign(&mut pol, &batch_dom.get_subdomain(2 * t));
        //         println!("Polynomial: {pol:?}\n");
        //         let reversed = fft(&pol, &batch_dom.get_subdomain(2 * t));
        //         println!("r: {s}\n\n{:?}\n", reversed);
        //         let missing = (n..n + t)
        //             .map(|i| batch_dom.get_root_of_unity(i))
        //             .collect::<Vec<_>>();
        //         let selected = (0..=2 * t).collect::<Vec<_>>();
        //         let finals = interpolate_on_single(&reversed, &selected, &missing, batch_dom);
        //         assert!(finals == evals);
        //         let mut fields = Vec::new();
        //         for _ in 0..=tx {
        //             fields.push(Polynomial::random(None, ty, rng));
        //         }
        //         BiVariatePoly { fields }
        //     }
        //     None => {
        let mut fields = Vec::new();
        for _ in 0..=tx {
            fields.push(Polynomial::random(None, ty, rng));
        }
        BiVariatePoly { fields }
    }

    pub fn random_with_rands<R>(
        secrets: Vec<Scalar>,
        tx: usize,
        ty: usize,
        sc: &Configuration,
        rng: &mut R,
    ) -> BiVariatePoly
    where
        R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
    {
        BiVariatePoly::random(sc, Some(secrets), tx, ty, rng)
    }

    pub fn degree_on_y(&self) -> usize {
        self.fields[0].fields().len() - 1
    }

    pub fn degree_on_x(&self) -> usize {
        self.fields.len() - 1
    }

    fn compute_powers(base: Scalar, degree: usize) -> Vec<Scalar> {
        let mut powers = vec![Scalar::one(); degree + 1];
        for i in 1..=degree {
            powers[i] = powers[i - 1] * base;
        }
        powers
    }

    pub fn evaluate_on_y(&self, y: Scalar) -> Polynomial {
        let powers = Self::compute_powers(y, self.degree_on_y());

        let fields: Vec<Scalar> = self
            .fields
            .iter()
            .enumerate()
            .map(|(_, col)| {
                col.fields()
                    .iter()
                    .zip(&powers)
                    .map(|(val, &p)| p * val)
                    .sum()
            })
            .collect();

        Polynomial::from(fields)
    }

    pub fn evaluate_on_x(&self, x: Scalar) -> Polynomial {
        let powers = Self::compute_powers(x, self.degree_on_x());

        let mut fields = vec![Scalar::zero(); self.degree_on_y() + 1];

        for col in &self.fields {
            for (y, (val, power)) in col.fields().iter().zip(&powers).enumerate() {
                fields[y] += power * val;
            }
        }

        Polynomial::from(fields)
    }
}
