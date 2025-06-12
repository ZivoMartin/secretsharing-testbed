use serde::{Deserialize, Serialize};

use crate::crypto::{data_structures::encryption::Encryption, Commitment};

#[derive(Serialize, Deserialize)]
pub struct BroadcastReceiv {
    pub comm: Commitment,
    pub encs: Vec<Encryption>,
}
