use global_lib::messages::Algo;

use crate::crypto::{Commitment, Secret, Share};
use std::collections::HashMap;
type Set = HashMap<u16, Share>;
pub type CryptoSetIdentity = (u16, u16, Algo); // (n, t, algo)

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CryptoSet {
    identity: CryptoSetIdentity,
    comm: Option<Commitment>,
    secrets: Option<Vec<Secret>>,
    set: Set,
}

impl CryptoSet {
    pub fn new(identity: CryptoSetIdentity) -> CryptoSet {
        CryptoSet {
            identity,
            ..Default::default()
        }
    }

    pub fn extract(&self) -> Self {
        Self {
            identity: self.identity,
            comm: Some(self.get_comm().clone()),
            secrets: self.get_secrets().clone(),
            set: self.set().clone(),
        }
    }

    pub fn identity(&self) -> CryptoSetIdentity {
        self.identity
    }

    pub fn set(&self) -> &Set {
        &self.set
    }

    pub fn clear(&mut self) {
        self.set.clear();
        self.comm = None;
    }

    pub fn contains(&self, i: u16) -> bool {
        self.set.contains_key(&i)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u16 {
        self.set.len() as u16
    }

    pub fn get(&self, i: u16) -> &Share {
        self.set.get(&i).as_ref().unwrap_or_else(|| {
            panic!(
                "SHARE_SET: Out of bound, index: {i}, length: {}",
                self.set.len()
            )
        })
    }

    pub fn set_shares(&mut self, share: Vec<Share>) {
        share.into_iter().for_each(|s| self.new_share(s))
    }

    pub fn throw(&mut self, i: u16) -> Share {
        self.set.remove(&i).unwrap()
    }

    pub fn new_share(&mut self, share: Share) {
        self.set.insert(share.index(), share);
    }

    pub fn set_comm(&mut self, comm: Commitment) {
        self.comm = Some(comm)
    }

    pub fn get_comm_mut(&mut self) -> &mut Commitment {
        self.comm.as_mut().unwrap()
    }

    pub fn get_comm(&self) -> &Commitment {
        self.comm.as_ref().unwrap()
    }

    pub fn has_comm(&self) -> bool {
        self.comm.is_some()
    }

    pub fn get_secrets(&self) -> &Option<Vec<Secret>> {
        &self.secrets
    }

    pub fn set_secrets(&mut self, secrets: Vec<Secret>) {
        self.secrets = Some(secrets);
    }
}
