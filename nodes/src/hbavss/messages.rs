use blsttc::{serde_impl::SerdeSecret, SecretKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Complaint {
    pub index: u16,
    pub pkey: SerdeSecret<SecretKey>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Assist {
    pub index: u16,
    pub pkey: SerdeSecret<SecretKey>,
}
