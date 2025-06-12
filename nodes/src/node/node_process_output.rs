use global_lib::config_treatment::result_fields::ResultDuration;

use crate::{crypto::crypto_set::CryptoSet, system::summaries::Summaries};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NodeProcessOutput {
    pub result: ResultDuration,
    pub share_set: Option<CryptoSet>,
    pub summaries: Summaries,
}

impl NodeProcessOutput {
    pub fn new(result: ResultDuration, summaries: Summaries, share_set: Option<CryptoSet>) -> Self {
        Self {
            result,
            summaries,
            share_set,
        }
    }
}
