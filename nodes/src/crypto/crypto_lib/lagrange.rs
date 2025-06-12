use super::{
    evaluation_domain::BatchEvaluationDomain,
    fft::{fft, fft_assign},
    polynomials::{accumulator_poly, poly_differentiate, poly_eval, poly_mul_slow},
};
use blstrs::Scalar;
use ff::{BatchInvert, Field};
use more_asserts::{assert_gt, debug_assert_le};
use std::ops::{Mul, MulAssign};

// TODO: We do not want this hardcoded here, most likely.
const FFT_THRESH: usize = 64;

/// Returns all the $N$ Lagrange coefficients for the interpolating set $T = \{\omega^0, \omega^1, \ldots, \omega^{N-1}\}$,
/// where $\omega$ is an $N$th root of unity and $N$ is the size of `dom`.
///
/// Much faster than calling `lagrange_coefficients` on this set, since each Lagrange coefficient
/// has a nice closed-form formula.
///
/// Specifically, if $f(X) = \sum_{i = 0}^{N-1} \ell_i(X) f(\omega^i)$, then expanding $\ell_i(X)$
/// gives $\ell_i(X) = \frac{(1-X^N) \omega^i}{N (\omega^i - X)}$.
///
/// For $X = \alpha$ we get $\ell_i(\alpha) = \frac{(1-\alpha^N) N^{-1} \omega^i}{(\omega^i - \alpha)}$.
///
/// (See https://ethresear.ch/t/kate-commitments-from-the-lagrange-basis-without-ffts/6950/2)
#[allow(non_snake_case)]
pub fn all_n_lagrange_coefficients(dom: &BatchEvaluationDomain, alpha: &Scalar) -> Vec<Scalar> {
    let alpha_to_N = alpha.pow_vartime([dom.N() as u64]); // \alpha^N
    let N_inverse = dom.get_subdomain(dom.N()).N_inverse; // N^{-1}
    let one_minus_alpha_to_N = Scalar::one() - alpha_to_N; // 1 - \alpha^N

    let lhs_numerator = N_inverse * one_minus_alpha_to_N; // (1 - \alpha^N) / N
    let omegas = dom.get_all_roots_of_unity(); // \omega^i, for all i
    let mut denominators = omegas.clone(); // clone
    for denom in denominators[..dom.N()].iter_mut() {
        *denom -= alpha // \omega^i - \alpha
    }

    denominators.batch_invert(); // (\omega^i - \alpha)^{-1}

    debug_assert_eq!(denominators.len(), dom.N());

    let mut coeffs = Vec::with_capacity(dom.N());

    for i in 0..dom.N() {
        // i.e., (1 - \alpha^N * \omega^i) / (N (\omega^i - \alpha))
        coeffs.push(lhs_numerator * omegas[i] * denominators[i])
    }
    coeffs
}

/// Returns the $|T|$ Lagrange coefficients $\ell_i = \prod_{j \in T, j \ne i} \frac{0 - \omega^j}{\omega^i - \omega_j}
/// using the $O(|T| \log^2{|T|})$ algorithm from [TCZ+20], where $\omega$ is an $N$th primitive
/// root of unity (see below for $N$).
///
/// Assumes that the batch evaluation domain in `dom` has all the $N$th roots of unity where $N = 2^k$.
///
/// `T` contains player identifiers, which are numbers from 0 to `N - 1` (inclusive).
/// The player with identifier $i$ is associated with $\omega^i$.
///
/// [TCZ+20]: **Towards Scalable Threshold Cryptosystems**, by Alin Tomescu and Robert Chen and
/// Yiming Zheng and Ittai Abraham and Benny Pinkas and Guy Golan Gueta and Srinivas Devadas,
/// *in IEEE S\&P'20*, 2020
#[allow(non_snake_case)]
pub fn lagrange_coefficients_at_zero(dom: &BatchEvaluationDomain, T: &[usize]) -> Vec<Scalar> {
    let N = dom.N();
    let t = T.len();
    assert_gt!(N, 1);
    // Technically, the accumulator poly has degree t, so we need to evaluate it on t+1 points, which
    // will be a problem when t = N, because the evaluation domain will be of size N, not N+1. However,
    // we handle this in `accumulator_poly_helper`
    debug_assert_le!(t, N);

    // Z(X) = \prod_{i in T} (X - \omega^i)
    let mut Z = accumulator_poly_helper(dom, T);

    // The set of $\omega_i$'s for all $i\in [0, N)$.
    let omegas = dom.get_all_roots_of_unity();

    // Let $Z_i(X) = Z(X) / (X - \omega^i)$. The variable below stores $Z_i(0) = - Z(0) / \omega^i$ for all $i\in T$.
    //
    // NOTE: This could be computed in parallel by (it would require re-computing $Z(0)$ manually as
    // $(-1)^{|T|} \prod_i T[i]$) but will not save any meaningful time (4% of the time is spent here).
    let Z_i_at_0 = compute_numerators_at_zero(omegas, T, &Z[0]);

    // Compute Z'(X), in place, overwriting Z(X)
    poly_differentiate(&mut Z);

    // Compute $Z'(\omega^i)$ for all $i\in [0, N)$, in place, overwriting $Z'(X)$.
    // (We only need $t$ of them, but computing all of them via an FFT is faster than computing them
    // via a multipoint evaluation.)
    //
    // NOTE: The FFT implementation could be parallelized, but only 17.7% of the time is spent here.
    fft_assign(&mut Z, &dom.get_subdomain(N));

    // Use batch inversion when computing the denominators 1 / Z'(\omega_i) (saves 3 ms)
    let mut denominators = Vec::with_capacity(T.len());
    for i in 0..T.len() {
        denominators.push(Z[T[i]]);
    }
    denominators.batch_invert();

    for i in 0..T.len() {
        Z[i] = Z_i_at_0[i].mul(denominators[i]);
    }

    Z.truncate(t);

    Z
}

/// Computes $Z(X) = \prod_{i \in T} (X - \omega^i)$.
#[allow(non_snake_case)]
fn accumulator_poly_helper(dom: &BatchEvaluationDomain, T: &[usize]) -> Vec<Scalar> {
    let omegas = dom.get_all_roots_of_unity();

    // Build the subset of $\omega_i$'s for all $i\in T$.
    let mut set = Vec::with_capacity(T.len());
    for &s in T {
        set.push(omegas[s]);
    }

    // TODO(Perf): This is the performance bottleneck: 75.58% of the time is spent here.
    //
    // Let $Z(X) = \prod_{i \in T} (X - \omega^i)$
    //
    // We handle a nasty edge case here: when doing N out of N interpolation, with N = 2^k, the batch
    // evaluation domain will have N roots of unity, but the degree of the accumulator poly will be
    // N+1. This will trigger an error inside `accumulator_poly` when doing the last FFT-based
    // multiplication, which would require an FFT evaluation domain of size 2N which is not available.
    //
    // To fix this, we handle this case separately by splitting the accumulator poly into an `lhs`
    // of degree `N` which can be safely interpolated with `accumulator_poly` and an `rhs` of degree
    // 1. We then multiply the two together. We do not care about any performance implications of this
    // since we will never use N-out-of-N interpolation.
    //
    // We do this to avoid complicating our Lagrange coefficients API and our BatchEvaluationDomain
    // API.
    if set.len() < dom.N() {
        accumulator_poly(&set, dom, FFT_THRESH)
    } else {
        let last = set.pop().unwrap();

        let lhs = accumulator_poly(&set, dom, FFT_THRESH);
        let rhs = accumulator_poly(&[last], dom, FFT_THRESH);

        poly_mul_slow(&rhs, &lhs)
    }
}

/// Let $Z_i(X) = Z(X) / (X - \omega^i)$. Returns a vector of $Z_i(0)$'s, for all $i\in T$.
#[allow(non_snake_case)]
fn compute_numerators_at_zero(omegas: &[Scalar], ids: &[usize], Z_0: &Scalar) -> Vec<Scalar> {
    let N = omegas.len();

    let mut numerators = Vec::with_capacity(ids.len());

    for &i in ids {
        /*
         * Recall that:
         *  a) Inverses can be computed fast as: (\omega^k)^{-1} = \omega^{-k} = \omega^N \omega^{-k} = \omega^{N-k}
         *  b) Negations can be computed fast as: -\omega^k = \omega^{k + N/2}
         *
         * So, (0 - \omega^i)^{-1} = (\omega^{i + N/2})^{-1} = \omega^{N - (i + N/2)} = \omega^{N/2 - i}
         * If N/2 < i, then you wrap around to N + N/2 - i.
         */

        let idx = if N / 2 < i { N + N / 2 - i } else { N / 2 - i };

        numerators.push(Z_0 * omegas[idx]);
    }

    debug_assert_eq!(numerators.len(), ids.len());

    numerators
}

/// Like `lagrange_coefficients_at_zero`, but instead of returning $\ell_i(X)$ evaluated at zero,
/// returns $\ell_i(\alpha$.
///
/// Recall that: $\ell_i(X) = \prod_{j \in T, j \ne i} \frac{X - \omega^j}{\omega^i - \omega_j}
#[allow(non_snake_case)]
pub fn lagrange_coefficients(
    dom: &BatchEvaluationDomain,
    T: &[usize],
    alphas: &[Scalar],
) -> Vec<Vec<Scalar>> {
    let N = dom.N();
    let t = T.len();
    assert_gt!(N, 1);
    // See comments in `lagrange_coefficients_at_zero` about this.
    debug_assert_le!(t, N);

    // The set of $\omega_i$'s for all $i\in [0, N)$.
    let omegas = dom.get_all_roots_of_unity();

    // Let $Z(X) = \prod_{i \in T} (X - \omega^i)$
    let mut Z = accumulator_poly_helper(dom, T);
    let Z_Saved = Z.clone();

    // Compute Z'(X), in place, overwriting Z(X)
    poly_differentiate(&mut Z);

    // Compute $Z'(\omega^i)$ for all $i\in [0, N)$, in place, overwriting $Z'(X)$.
    // (We only need $t$ of them, but computing all of them via an FFT is faster than computing them
    // via a multipoint evaluation.)
    //
    // NOTE: The FFT implementation could be parallelized, but only 17.7% of the time is spent here.
    fft_assign(&mut Z, &dom.get_subdomain(N));

    // Use batch inversion when computing the denominators 1 / Z'(\omega_i) (saves 3 ms)
    let mut denominators = Vec::with_capacity(T.len());
    for i in 0..T.len() {
        denominators.push(Z[T[i]]);
    }
    denominators.batch_invert();

    alphas
        .iter()
        .map(|alpha| {
            // Let $Z_i(X) = Z(X) / (X - \omega^i)$. The variable below stores $Z_i(\alpha) = Z(\alpha) / (\alpha - \omega^i)$ for all $i\in T$.
            let mut Z = Z.clone();
            let Z_i_at_alpha = compute_numerators(&Z_Saved, omegas, T, alpha);
            for i in 0..T.len() {
                Z[i] = Z_i_at_alpha[i].mul(denominators[i]);
            }
            Z.truncate(t);
            Z
        })
        .collect()
}

/// Let $Z_i(X) = Z(X) / (X - \omega^i)$. Returns a vector of $Z_i(\alpha)$'s, for all $i\in T$.
#[allow(non_snake_case)]
fn compute_numerators(
    Z: &Vec<Scalar>,
    omegas: &[Scalar],
    ids: &[usize],
    alpha: &Scalar,
) -> Vec<Scalar> {
    let mut numerators = Vec::with_capacity(ids.len());

    // Z(\alpha)
    let Z_of_alpha = poly_eval(Z, alpha);

    for &i in ids {
        // \alpha - \omega^i
        numerators.push(alpha - omegas[i]);
    }

    // (\alpha - \omega^i)^{-1}
    numerators.batch_invert();

    for num in numerators.iter_mut() {
        // Z(\alpha) / (\alpha - \omega^i)^{-1}
        num.mul_assign(Z_of_alpha);
    }

    numerators
}

#[allow(non_snake_case)]
pub fn all_lagrange_denominators(batch_dom: &BatchEvaluationDomain, n: usize) -> Vec<Scalar> {
    // A(X) = \prod_{i \in [0, n-1]} (X - \omega^i)
    let mut A = accumulator_poly_helper(batch_dom, (0..n).collect::<Vec<usize>>().as_slice());

    // A'(X) = \sum_{i \in [0, n-1]} \prod_{j \ne i, j \in [0, n-1]} (X - \omega^j)
    poly_differentiate(&mut A);

    // A'(\omega^i) = \prod_{j\ne i, j \in [n] } (\omega^i - \omega^j)
    let mut denoms = fft(&A, &batch_dom.get_subdomain(n));

    denoms.truncate(n);

    denoms.batch_invert();

    denoms
}
