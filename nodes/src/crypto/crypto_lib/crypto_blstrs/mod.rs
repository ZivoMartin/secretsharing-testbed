extern crate core;
use blstrs;
use blstrs::{G1Projective, G2Projective, Scalar};
pub use ff;
use ff::Field;
pub use group;
use group::Group;
use std::ops::{AddAssign, Mul, MulAssign, Sub, SubAssign};

pub mod poly_commit;
pub mod polynomial;

pub fn eval<G, S>(coeff: &[G], x: &S, zero: G) -> G
where
    G: Clone,
    for<'a> G: AddAssign<&'a G>,
    for<'a> G: MulAssign<&'a S>,
{
    let mut b = match coeff.last() {
        None => return zero,
        Some(b) => b.clone(),
    };
    for c in coeff.iter().rev().skip(1) {
        b *= x;
        b += c;
    }
    b
}

#[allow(non_snake_case)]
#[allow(unused)]
pub fn blstrs_eval_G1Projective(coeff: &[G1Projective], x: &Scalar) -> G1Projective {
    match coeff.len() {
        0 => G1Projective::identity(),
        1 => coeff[0],
        _ => {
            let mut scalars = Vec::with_capacity(coeff.len() - 1);
            let mut xx = *x;
            for _ in 1..coeff.len() {
                scalars.push(xx);
                xx *= x;
            }
            G1Projective::multi_exp(&coeff[1..], scalars.as_slice()) + coeff[0]
        }
    }
}

#[allow(non_snake_case)]
#[allow(unused)]
pub fn blstrs_eval_G2Projective(coeff: &[G2Projective], x: &Scalar) -> G2Projective {
    match coeff.len() {
        0 => G2Projective::identity(),
        1 => coeff[0],
        _ => {
            let mut scalars = Vec::with_capacity(coeff.len() - 1);
            let mut xx = *x;
            for _ in 1..coeff.len() {
                scalars.push(xx);
                xx *= x;
            }
            G2Projective::multi_exp(&coeff[1..], scalars.as_slice()) + coeff[0]
        }
    }
}

fn lagrange_helper(k: &Scalar, j: &Scalar, x: &Scalar) -> Scalar {
    x.sub(k).mul(j.sub(k).invert().unwrap())
}

fn lagrange(idxes: &[Scalar], j: &Scalar, x: &Scalar) -> Scalar {
    idxes
        .iter()
        .filter_map(|k| {
            if k != j {
                Some(lagrange_helper(k, j, x))
            } else {
                None
            }
        })
        .product()
}

pub fn interpolate<G, S>(xs: &[S], ys: Vec<G>, inv: fn(S) -> S, identity: G) -> Vec<G>
where
    G: Clone + SubAssign<G>,
    for<'a> G: MulAssign<&'a S>,
    for<'a, 'b> &'a S: Sub<&'b S, Output = S>,
    for<'a, 'b> &'a G: Sub<&'b G, Output = G>,
{
    assert_eq!(xs.len(), ys.len(), "xs and ys are not same length!");
    let mut polys: Vec<_> = ys.into_iter().map(|g| vec![g]).collect();
    let mul_poly = |poly: &mut Vec<G>, x| {
        poly.push(identity.clone());
        for l in (0..poly.len()).rev() {
            poly[l] *= x;
            let (m, overflow) = l.overflowing_sub(1);
            if !overflow {
                poly[l] = &poly[l] - &poly[m];
            }
        }
    };
    for j in 1..polys.len() {
        for (k, i) in (0..j).rev().enumerate() {
            let mut poly_j = polys[j - k].clone();
            mul_poly(&mut poly_j, &xs[i]);
            mul_poly(&mut polys[i], &xs[j]);
            let diff = inv(&xs[j] - &xs[i]);
            polys[i]
                .iter_mut()
                .zip(poly_j.into_iter())
                .for_each(|(x, y)| {
                    *x -= y;
                    *x *= &diff;
                });
        }
    }
    if polys.is_empty() {
        vec![] // When no points given; P(x) = 0
    } else {
        polys.swap_remove(0)
    }
}

#[allow(non_snake_case)]
#[allow(unused)]
pub fn blstrs_lagrange_G1Projective<T: AsRef<Vec<G1Projective>>>(
    xs: &[Scalar],
    ys: T,
    z: &Scalar,
) -> G1Projective {
    let xs: Vec<_> = xs.iter().map(|x| lagrange(xs, x, z)).collect();
    G1Projective::multi_exp(ys.as_ref().as_slice(), xs.as_slice())
}

#[allow(non_snake_case)]
#[allow(unused)]
pub fn blstrs_lagrange_G2Projective(
    xs: &[Scalar],
    ys: &[G2Projective],
    z: &Scalar,
) -> G2Projective {
    let xs: Vec<_> = xs.iter().map(|x| lagrange(xs, x, z)).collect();
    G2Projective::multi_exp(ys, xs.as_slice())
}
