use super::broadcast_message_types::{BroadcastMessageType, Transcript};
use crate::{
    crypto::data_structures::reed_solomon_code::{reed_solomon_encode, RSDecoder, RSDecoderData},
    node::node::Node,
};
use global_lib::{enc, messages::BroadcastCommand, Wrapped};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

type Bytes = Vec<u8>;

#[derive(PartialEq)]
enum ReadyState {
    None,
    Wait,
    Ready,
}

struct MessageManager {
    hash: Bytes,
    my_share: Bytes,
    decoder: RSDecoder,
    echo: u8,
    ready: u8,
    ready_state: ReadyState,
    node: Wrapped<Node>,
    kind: BroadcastMessageType,
}

impl MessageManager {
    async fn new(node: Wrapped<Node>, kind: BroadcastMessageType, decoder: RSDecoder) -> Self {
        Self {
            node,
            kind,
            decoder,
            ready_state: ReadyState::None,
            my_share: Vec::new(),
            echo: 0,
            ready: 0,
            hash: Vec::new(),
        }
    }

    async fn encode_message(
        node: &Wrapped<Node>,
        message: Bytes,
    ) -> (Vec<Bytes>, RSDecoderData, Bytes) {
        let (n, t) = {
            let node = node.lock().await;
            (node.n() as usize, node.n() as usize / 3)
        };
        let hash = Self::get_message_hash(&message);
        let (shares, datas) = reed_solomon_encode(message, n, t + 1);
        (shares, datas, hash)
    }

    async fn new_from_message(
        node: Wrapped<Node>,
        kind: BroadcastMessageType,
        message: Bytes,
    ) -> Self {
        let (shares, datas, hash) = Self::encode_message(&node, message).await;
        let mut mem = Self::new(node, kind, RSDecoder::new(datas)).await;
        mem.set_message(kind, shares, datas, hash).await;
        mem
    }

    async fn set_message(
        &mut self,
        kind: BroadcastMessageType,
        shares: Vec<Bytes>,
        datas: RSDecoderData,
        hash: Bytes,
    ) {
        self.hash = hash.clone();
        let mut tr = Transcript {
            kind,
            datas,
            hash,
            i: self.node.lock().await.index(),
            share: Vec::new(),
        };
        let mut my_share = Vec::new();
        {
            let mut node = self.node.lock().await;
            for (i, share) in shares.into_iter().enumerate() {
                if i == node.uindex() {
                    my_share = share;
                    continue;
                }
                tr.share = share;
                let msg = Arc::new(enc!(Broadcast, BroadcastCommand::Echo, tr));
                node.contact(i, msg);
            }
            node.index()
        };
        tr.share = my_share;
        self.add_echo(tr).await;
    }

    async fn new_from_echo(node: Wrapped<Node>, tr: Transcript) -> Self {
        let mut res = Self::new(node, tr.kind, RSDecoder::new(tr.datas)).await;
        res.add_echo(tr).await;
        res
    }

    async fn new_from_ready(node: Wrapped<Node>, tr: Transcript) -> Self {
        let mut res = Self::new(node, tr.kind, RSDecoder::new(tr.datas)).await;
        res.add_ready(tr).await;
        res
    }

    async fn broadcast_ready(&mut self) {
        if !self.have_my_share() {
            self.ready_state = ReadyState::None;
        }
        let node_i = self.node.lock().await.index();
        if self.ready_state != ReadyState::Ready {
            let tr = Transcript {
                hash: self.hash.clone(),
                i: node_i,
                datas: self.datas(),
                share: self.my_share.clone(),
                kind: self.kind,
            };
            let msg = enc!(Broadcast, BroadcastCommand::Ready, tr);
            self.node.lock().await.broadcast(msg, false).await;
            self.ready_state = ReadyState::Ready;
        }
    }

    fn datas(&self) -> RSDecoderData {
        self.decoder.datas()
    }

    fn have_my_share(&self) -> bool {
        !self.my_share.is_empty()
    }

    fn get_message_hash(message: &Bytes) -> Bytes {
        let mut hasher = Sha256::new();
        hasher.update(message);
        hasher.finalize().to_vec()
    }

    fn check_hash(&self, hash: Bytes) -> Result<(), ()> {
        if self.hash == hash {
            Ok(())
        } else {
            Err(())
        }
    }

    fn set_hash(&mut self, hash: Bytes) -> Result<(), ()> {
        if self.hash.is_empty() {
            self.hash = hash;
            Ok(())
        } else {
            self.check_hash(hash)
        }
    }

    async fn add_echo(&mut self, mut tr: Transcript) {
        self.set_hash(tr.hash.clone()).unwrap();
        self.echo += 1;
        // NOTE: datas.t reprensents the number of share needed to reconstruct (t+1), as we want 2*t + 1 echo to broadcast ready we will need datas.t * 2 - 1 echo that is equal at 2*t+1.
        if self.echo as usize == 2 * self.datas().t - 1 {
            self.broadcast_ready().await
        } else if !self.have_my_share() {
            tr.i = self.node.lock().await.index();
            self.my_share = tr.share.clone();
            self.add_ready(tr).await;
            if self.ready_state == ReadyState::Wait {
                self.broadcast_ready().await;
            }
        }
    }

    async fn add_ready(&mut self, tr: Transcript) -> bool {
        let Transcript { hash, share, i, .. } = tr;
        if !share.is_empty() {
            self.set_hash(hash).unwrap();
            assert!(share.len() == self.datas().pow_2_size);
            self.decoder.add_recovery_share(i as usize, &share);
        }
        self.ready += 1;
        let t = self.datas().t - 1;
        let output = self.ready as usize == t * 2 + 1;
        if self.ready as usize == t + 1 {
            self.broadcast_ready().await;
        } else if output {
            self.output().await
        }
        output
    }

    async fn output(&mut self) {
        let message = self.decoder.decode();
        let hash = Self::get_message_hash(&message);
        self.check_hash(hash).unwrap();
        let message = self.kind.get_node_message(message);
        let node = self.node.clone();
        tokio::spawn(async move {
            Node::wait_and_send(&node, message).await;
        });
    }
}

pub struct BroadcastMemory {
    node: Wrapped<Node>,
    proposed: HashMap<BroadcastMessageType, Option<MessageManager>>,
}

impl BroadcastMemory {
    pub fn new(node: Wrapped<Node>) -> Self {
        Self {
            node,
            proposed: HashMap::new(),
        }
    }

    pub async fn propose(&mut self, kind: BroadcastMessageType, bytes: Bytes) {
        match self.proposed.get_mut(&kind) {
            Some(manager) if manager.is_some() => {
                let (shares, datas, hash) = MessageManager::encode_message(&self.node, bytes).await;
                manager
                    .as_mut()
                    .unwrap()
                    .set_message(kind, shares, datas, hash)
                    .await
            }
            None => {
                let _ = self.proposed.insert(
                    kind,
                    Some(MessageManager::new_from_message(self.node.clone(), kind, bytes).await),
                );
            }
            _ => (),
        }
    }

    pub async fn add_ready(&mut self, tr: Transcript) {
        let kind = tr.kind;
        if match self.proposed.get_mut(&kind) {
            Some(manager) if manager.is_some() => manager.as_mut().unwrap().add_ready(tr).await,
            None => {
                let _ = self.proposed.insert(
                    kind,
                    Some(MessageManager::new_from_ready(self.node.clone(), tr).await),
                );
                false
            }
            _ => false,
        } {
            self.proposed.insert(kind, None);
        }
    }

    pub async fn add_echo(&mut self, tr: Transcript) {
        let kind = tr.kind;
        match self.proposed.get_mut(&kind) {
            Some(manager) if manager.is_some() => manager.as_mut().unwrap().add_echo(tr).await,
            None => {
                let _ = self.proposed.insert(
                    kind,
                    Some(MessageManager::new_from_echo(self.node.clone(), tr).await),
                );
            }
            _ => (),
        }
    }
}
