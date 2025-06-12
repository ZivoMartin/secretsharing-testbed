use crate::crypto::{Commitment, Share};
use blstrs::Scalar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct SendMessage {
    pub root: Vec<u8>,
    pub comms: (Vec<Commitment>, Commitment), // recover and shares
    pub evals: Vec<Share>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct EchoMessage {
    pub root: Vec<u8>,
    pub comm: Commitment, // recover and shares
    pub evals: Vec<(Scalar, Scalar)>,
    pub sender: usize,
}
