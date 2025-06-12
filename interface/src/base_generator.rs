use blstrs::G1Projective;
use group::Group;

const DST_PVSS_PUBLIC_PARAMS_GENERATION: &[u8; 35] = b"AptosPvssPublicParametersGeneration";
pub fn generate_random_base() -> [G1Projective; 2] {
    let seed = b"hello";
    let g = G1Projective::generator();
    let h = G1Projective::hash_to_curve(seed, DST_PVSS_PUBLIC_PARAMS_GENERATION.as_slice(), b"h");
    [g, h]
}
