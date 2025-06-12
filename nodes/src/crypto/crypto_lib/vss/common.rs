use aptos_crypto::{
    bls12381::{PrivateKey, PublicKey},
    ed25519::{Ed25519PrivateKey, Ed25519Signature},
    test_utils::{KeyPair, TEST_SEED},
    Uniform,
};
use blstrs::{G1Projective, Scalar};
use group::Group;
use std::ops::Mul;

use rand::{distributions, prelude::Distribution, rngs::StdRng, thread_rng};
use rand_core::SeedableRng;

use crate::{
    crypto::{
        crypto_lib::{
            evaluation_domain::BatchEvaluationDomain, fft::fft_assign,
            lagrange::all_lagrange_denominators, random_scalars,
        },
        data_structures::commitment::Commitment,
    },
    node::configuration::Configuration,
};

/// Return a random scalar within a small range [0,n)
pub fn random_scalar_range<R>(mut rng: &mut R, u: u64) -> Scalar
where
    R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
{
    let die = distributions::Uniform::from(0..u);
    let val = die.sample(&mut rng);
    Scalar::from(val)
}

pub fn random_scalars_range<R>(mut rng: &mut R, u: u64, n: usize) -> Vec<Scalar>
where
    R: rand_core::RngCore + rand::Rng + rand_core::CryptoRng + rand::CryptoRng,
{
    let mut v = Vec::with_capacity(n);

    for _ in 0..n {
        v.push(random_scalar_range(&mut rng, u));
    }
    v
}

// #[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
// pub struct Share {
//     pub(crate) share: [Scalar; 2],
// }

// impl Share {
//     pub fn get(&self) -> &[Scalar] {
//         self.share.as_slice()
//     }

//     pub fn identity() -> Share {
//         let share = [Scalar::from(1), Scalar::from(1)];
//         Share { share }
//     }
// }

/// Checks that the committed degred is low
pub fn low_deg_test(comms: &Commitment, sc: &Configuration) -> bool {
    // If the degree is n-1, then the check is trivially true
    if sc.get_threshold() == sc.n() as usize {
        return true;
    }

    let mut rng = thread_rng();
    let vf = get_dual_code_word(
        sc.get_threshold() - 1,
        sc.get_batch_evaluation_domain(),
        sc.n() as usize,
        &mut rng,
    );
    for comm in comms.all() {
        let ip = G1Projective::multi_exp(comm, vf.as_ref());
        if !ip.eq(&G1Projective::identity()) {
            return false;
        }
    }
    true
}

pub fn get_dual_code_word<R: rand_core::RngCore + rand_core::CryptoRng>(
    deg: usize,
    batch_dom: &BatchEvaluationDomain,
    n: usize,
    mut rng: &mut R,
) -> Vec<Scalar> {
    // The degree-(t-1) polynomial p(X) that shares our secret
    // So, deg = t-1 => t = deg + 1
    // The "dual" polynomial f(X) of degree n - t - 1 = n - (deg + 1) - 1 = n - deg - 2
    let mut f = random_scalars(n - deg - 2, &mut rng);

    // Compute f(\omega^i) for all i's
    let dom = batch_dom.get_subdomain(n);
    fft_assign(&mut f, &dom);
    f.truncate(n);

    // Compute v_i = 1 / \prod_{j \ne i, j \in [0, n-1]} (\omega^i - \omega^j), for all i's
    let v = all_lagrange_denominators(batch_dom, n);

    // Compute v_i * f(\omega^i), for all i's
    let vf = f
        .iter()
        .zip(v.iter())
        .map(|(v, f)| v.mul(f))
        .collect::<Vec<Scalar>>();

    vf
}

pub fn sign_verified_deal(sig_key: Ed25519PrivateKey, msg: Vec<u8>) -> Option<Ed25519Signature> {
    return Some(sig_key.sign_arbitrary_message(msg.as_slice()));
}

// Helper function to generate N bls12381 private keys.
pub fn generate_bls_sig_keys(n: usize) -> Vec<KeyPair<PrivateKey, PublicKey>> {
    let mut rng = StdRng::from_seed(TEST_SEED);
    (0..n)
        .map(|_| KeyPair::<PrivateKey, PublicKey>::generate(&mut rng))
        .collect()
}
