use super::{
    enc_dec::{decrypt, get_key},
    messages::{EchoMessage, ForwardMessage, ForwardTag, ProposeMessage, VoteMessage},
    Bytes,
};
use crate::{
    crypto::data_structures::{
        merkle_tree::{compute_root, hash_leafs, verify, MHash, MProof, SerializableProof},
        reed_solomon_code::{reed_solomon_encode, RSDecoder, RSDecoderData},
    },
    node::{node::Node, node_message::NodeMessage},
};
use blstrs::Scalar;
use ff::Field;
use global_lib::Wrapped;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Secret {
    eval: Scalar,
    share: Bytes,
    proof: MProof,
    i: usize,
}

impl Secret {
    pub fn new(eval: Scalar, share: Bytes, proof: MProof, i: usize) -> Self {
        Self {
            eval,
            share,
            proof,
            i,
        }
    }

    pub fn extract(self) -> (usize, (Scalar, Vec<u8>, SerializableProof)) {
        (
            self.i,
            (
                self.eval,
                self.share,
                SerializableProof::from_proof(&self.proof),
            ),
        )
    }

    fn check(&self) -> Result<(), ()> {
        let merkle_leaf = self.compute_merkle_leaf();
        if !verify(&merkle_leaf, &self.proof) {
            Err(())
        } else {
            Ok(())
        }
    }

    fn compute_merkle_leaf(&self) -> Bytes {
        let mut s = self.share.clone();
        s.append(&mut get_key(self.i, &self.eval).to_bytes_be().to_vec());
        s
    }

    fn root(&self) -> MHash {
        self.proof.root()
    }
}

type SecretSet = HashMap<usize, Secret>;

pub struct Memory {
    node: Wrapped<Node>,
    is_ready: bool,
    voted: bool,
    n: usize,
    t: usize,
    i: usize,
    main_root: Option<MHash>,
    roots_proof: Vec<Option<MProof>>,
    datas: Option<RSDecoderData>,
    share_count: usize,
    vote_count: usize,
    secrets_set: HashMap<usize, SecretSet>,
}

impl Memory {
    pub async fn new(node: Wrapped<Node>) -> Self {
        let (n, _, i) = {
            let node = node.lock().await;
            (node.n() as usize, node.t() as usize + 1, node.uindex())
        };
        let mut secrets_set = HashMap::with_capacity(n);
        secrets_set.insert(i, HashMap::with_capacity(n));
        Self {
            node,
            n,
            t: n / 3,
            i,
            secrets_set,
            is_ready: false,
            voted: false,
            main_root: None,
            roots_proof: vec![None; n],
            datas: None,
            share_count: 0,
            vote_count: 0,
        }
    }

    fn check_root_proof(&mut self, root: &Bytes, proof: &MProof) {
        assert!(verify(root, proof));
        match self.main_root {
            Some(mr) => assert!(mr == proof.root()),
            None => self.main_root = Some(proof.root()),
        }
    }

    fn set_root_proof(&mut self, proof: MProof, i: usize) {
        match &self.roots_proof[i] {
            Some(mr) => assert!(*mr == proof),
            None => self.roots_proof[i] = Some(proof),
        }
    }

    async fn try_to_output(&mut self) {
        if self.is_ready {
            self.output().await;
        } else {
            self.is_ready = true
        }
    }

    fn decode(&self, i: usize) -> Bytes {
        let secrets = self.secrets_set.get(&i).unwrap();
        let mut decoder = RSDecoder::new(*self.datas.as_ref().unwrap());
        for (i, s) in secrets {
            decoder.add_recovery_share(*i, &s.share);
        }
        let d = self.n - 2 * self.t - 1;
        assert!(secrets.len() > d);
        let mut selected = Vec::with_capacity(d + 1);
        let evals = secrets
            .iter()
            .take(d + 1)
            .map(|(_, s)| {
                selected.push(s.i);
                s.eval
            })
            .collect::<Vec<_>>();

        let coeffs = selected
            .iter()
            .enumerate()
            .map(|(i, si)| {
                selected
                    .iter()
                    .enumerate()
                    .map(|(j, sj)| {
                        if i != j {
                            (Scalar::from(*si as u64) - Scalar::from(*sj as u64))
                                .invert()
                                .unwrap()
                        } else {
                            Scalar::zero()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let interpolated = (0..=self.n)
            .map(|i| {
                let eval = match selected.iter().position(|j| *j == i) {
                    Some(j) => evals[j],
                    None => {
                        let alpha = Scalar::from(i as u64);
                        let mut result = Scalar::zero();
                        for i in 0..selected.len() {
                            let mut term = evals[i];
                            for (j, &sj) in selected.iter().enumerate() {
                                if i != j {
                                    let numer = alpha - Scalar::from(sj as u64);
                                    term *= numer * coeffs[i][j]
                                }
                            }
                            result += term;
                        }
                        result
                    }
                };
                get_key(i, &eval)
            })
            .collect::<Vec<_>>();
        let recovered = decoder.compute_all_shares();
        let secret = decoder.compute_secret_from_shares(recovered);

        let root = compute_root(
            reed_solomon_encode(secret.clone(), self.n, self.n - 2 * self.t)
                .0
                .into_iter()
                .zip(interpolated.iter().skip(1))
                .map(|(mut s, k)| {
                    s.append(&mut k.to_bytes_be().to_vec());
                    s
                })
                .collect(),
        );
        assert!(root == secrets.iter().next().unwrap().1.root());
        decrypt(&secret, &interpolated[0])
    }

    async fn output(&mut self) {
        let share = self.decode(self.i);
        let msg = NodeMessage::SMDOutput(share);
        Node::wait_and_send(&self.node, msg).await;
    }

    async fn vote(&mut self) {
        assert!(!self.voted);
        self.voted = true;
        self.node
            .lock()
            .await
            .broadcast(VoteMessage::get_transcript(self.main_root.unwrap()), true)
            .await;
    }

    async fn add_vote(&mut self) {
        self.vote_count += 1;
        if self.vote_count == self.t + 1 && !self.voted {
            self.vote().await
        } else if self.vote_count == self.n - self.t {
            self.try_to_output().await
        }
    }

    fn get_my_secret_set_mut(&mut self) -> &mut SecretSet {
        self.secrets_set.get_mut(&self.i).unwrap()
    }

    async fn insert_secret(&mut self, secret: Secret) {
        self.get_my_secret_set_mut().insert(secret.i - 1, secret);
        self.share_count += 1;
        if self.share_count == self.n - self.t && !self.voted {
            self.vote().await
        } else if self.share_count == self.n - 2 * self.t {
            self.try_to_output().await
        }
    }

    pub async fn propose(&mut self, msg: ProposeMessage) {
        let (datas, evals, sp) = msg.extract();
        if self.datas.is_none() {
            self.datas = Some(datas)
        }
        let mut proofs = Vec::with_capacity(sp.len());
        let mut shares = Vec::with_capacity(sp.len());
        let mut roots = Vec::with_capacity(sp.len());
        for ((proof, share), eval) in sp.into_iter().zip(evals.iter()) {
            let mut s = share.clone();
            s.append(&mut get_key(self.i + 1, eval).to_bytes_be().to_vec());
            assert!(verify(&s, &proof));
            roots.push(proof.root().to_vec());
            shares.push(share);
            proofs.push(proof);
        }
        let roots_proofs = hash_leafs(roots);
        self.main_root = Some(roots_proofs[0].root());
        for (i, p) in roots_proofs.iter().enumerate() {
            self.set_root_proof(p.clone(), i);
        }
        let mut msp = None;
        {
            let mut node = self.node.lock().await;
            for (j, (((root_proof, proof), share), eval)) in roots_proofs
                .into_iter()
                .zip(proofs)
                .zip(shares)
                .zip(evals)
                .enumerate()
            {
                if j != self.i {
                    let msg =
                        EchoMessage::get_transcript(root_proof, share, proof, self.i, eval, datas);
                    node.contact(j, Arc::new(msg))
                } else {
                    msp = Some(Secret::new(eval, share, proof, j + 1));
                }
            }
        }
        self.insert_secret(msp.take().unwrap()).await;
    }

    pub async fn new_vote(&mut self, msg: VoteMessage) {
        match self.main_root {
            Some(root) => assert!(root == msg.vote),
            None => self.main_root = Some(msg.vote),
        }
        self.add_vote().await;
    }

    pub async fn new_echo(&mut self, msg: EchoMessage) {
        let (root_proof, share, proof, i, eval, datas) = msg.extract();
        if self.datas.is_none() {
            self.datas = Some(datas)
        }
        self.check_root_proof(&proof.root().to_vec(), &root_proof);
        let secret = Secret::new(eval, share, proof, i + 1);
        secret.check().unwrap();
        self.set_root_proof(root_proof, self.i);
        self.insert_secret(secret).await;
    }

    pub fn forward_decode(&mut self, msg: ForwardMessage) -> Bytes {
        let (tag, root_proof, shares, _) = msg.extract();
        for secret in shares.values() {
            secret.check().unwrap();
            assert!(verify(&secret.root().to_vec(), &root_proof));
        }
        self.secrets_set.insert(tag.i(), shares);
        self.decode(tag.i())
    }

    pub async fn forward_receiv(mem: Wrapped<Self>, msg: ForwardMessage) {
        let node = mem.lock().await.node.clone();
        let msg = msg.to_node_message(mem);
        tokio::spawn(async move { Node::try_wait_and_send(&node, msg).await });
    }

    pub async fn forward_request(&self, tag: ForwardTag) {
        let msg = ForwardMessage::get_transcript(
            tag,
            self.roots_proof[tag.i()].as_ref().unwrap().clone(),
            self.secrets_set.get(&tag.i()).unwrap().clone(),
            self.i,
        );
        self.node.lock().await.broadcast(msg, false).await
    }
}
