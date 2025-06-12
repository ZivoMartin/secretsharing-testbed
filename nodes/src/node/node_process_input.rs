use crate::crypto::{
    crypto_set::CryptoSet,
    data_structures::{
        keypair::{KeyPair, PublicKey},
        Base,
    },
};

use global_lib::{config_treatment::fields::Fields, network::Network, OpId};

pub struct NodeProcessInput {
    pub index: u16,
    pub id: OpId,
    pub fields: Fields,
    pub network: Network,
    pub public_keys: Vec<PublicKey>,
    pub shares: CryptoSet,
    pub keypair: KeyPair,
    pub dealer: u16,
    pub base: Base,
}

impl NodeProcessInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fields: Fields,
        id: OpId,
        index: u16,
        keypair: KeyPair,
        network: Network,
        public_keys: Vec<PublicKey>,
        shares: CryptoSet,
        dealer: u16,
        base: Base,
    ) -> Self {
        NodeProcessInput {
            index,
            id,
            fields,
            network,
            shares,
            public_keys,
            dealer,
            keypair,
            base,
        }
    }
}
