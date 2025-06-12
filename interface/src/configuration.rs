use crate::network::Network;
use global_lib::{config_treatment::fields::Fields, with_getters, OpId};

with_getters!(
    struct Configuration {
        fields: Fields,
        network: Network,
        id: OpId,
    }
);

impl Configuration {
    pub fn new(fields: Fields, id: OpId, network: Network) -> Self {
        Configuration {
            fields,
            id,
            network,
        }
    }
}
