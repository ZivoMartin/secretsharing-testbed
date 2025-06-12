use blstrs::G1Projective;
use serde::{Deserialize, Serialize};

use crate::crypto::{
    data_structures::{commitment::Commitment, encryption::Encryption},
    Share, Sign,
};

#[derive(Serialize, Deserialize)]
pub struct BroadcastReceiv {
    pub comm: Commitment,
    pub signs: Vec<(u16, Sign)>,
    pub shares: Vec<Share>,
    pub encs: Vec<Encryption>,
    pub missing_coms: Vec<Vec<G1Projective>>,
    pub missing: Vec<usize>,
}
