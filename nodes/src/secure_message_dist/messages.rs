use super::smd_memory::{Memory, Secret};
use crate::{
    crypto::data_structures::{
        merkle_tree::{MHash, MProof, SerializableProof},
        reed_solomon_code::RSDecoderData,
    },
    node::node_message::NodeMessage,
};
use blstrs::Scalar;
use global_lib::{enc, messages::SecureMsgDisCommand, Wrapped};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Copy, Clone, Serialize, Deserialize, Debug, Eq, PartialEq, Hash)]
pub enum ForwardTag {
    Complaint(usize),
    Assist(usize),
    Report(usize),
}

impl ForwardTag {
    pub fn i(self) -> usize {
        match self {
            Self::Complaint(i) => i,
            Self::Assist(i) => i,
            Self::Report(i) => i,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ForwardMessage {
    pub tag: ForwardTag,
    pub shares: HashMap<usize, (Scalar, Vec<u8>, SerializableProof)>,
    root_proof: SerializableProof,
    i: usize,
}

impl ForwardMessage {
    pub fn get_transcript(
        tag: ForwardTag,
        roots_proofs: MProof,
        shares: HashMap<usize, Secret>,
        i: usize,
    ) -> Vec<u8> {
        let tr = ForwardMessage {
            root_proof: SerializableProof::from_proof(&roots_proofs),
            tag,
            shares: shares.into_values().map(|s| s.extract()).collect(),
            i,
        };
        enc!(SecureMsgDis, SecureMsgDisCommand::Forward, tr)
    }

    pub fn to_node_message(self, mem: Wrapped<Memory>) -> NodeMessage {
        match self.tag {
            ForwardTag::Complaint(_) => NodeMessage::SMDForwardLightWeightComplaint(mem, self),
            ForwardTag::Assist(_) => NodeMessage::SMDForwardLightWeightAssist(mem, self),
            ForwardTag::Report(_) => NodeMessage::SMDForwardLightWeightReport(mem, self),
        }
    }

    pub fn extract(self) -> (ForwardTag, MProof, HashMap<usize, Secret>, usize) {
        (
            self.tag,
            self.root_proof.to_proof(),
            self.shares
                .into_iter()
                .map(|(i, (eval, s, p))| (i - 1, Secret::new(eval, s, p.to_proof(), i)))
                .collect(),
            self.i,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct VoteMessage {
    pub vote: MHash,
}

impl VoteMessage {
    pub fn get_transcript(vote: MHash) -> Vec<u8> {
        let tr = VoteMessage { vote };
        enc!(SecureMsgDis, SecureMsgDisCommand::Vote, tr)
    }
}

#[derive(Serialize, Deserialize)]
pub struct EchoMessage {
    datas: RSDecoderData,
    root_proof: SerializableProof,
    share: Vec<u8>,
    proof: SerializableProof,
    eval: Scalar,
    i: usize,
}

impl EchoMessage {
    pub fn get_transcript(
        root_proof: MProof,
        share: Vec<u8>,
        proof: MProof,
        i: usize,
        eval: Scalar,
        datas: RSDecoderData,
    ) -> Vec<u8> {
        let tr = EchoMessage {
            datas,
            root_proof: SerializableProof::from_proof(&root_proof),
            proof: SerializableProof::from_proof(&proof),
            share,
            eval,
            i,
        };
        enc!(SecureMsgDis, SecureMsgDisCommand::Echo, tr)
    }

    pub fn extract(self) -> (MProof, Vec<u8>, MProof, usize, Scalar, RSDecoderData) {
        (
            self.root_proof.to_proof(),
            self.share,
            self.proof.to_proof(),
            self.i,
            self.eval,
            self.datas,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProposeMessage {
    datas: RSDecoderData,
    shares_and_proofs: Vec<(SerializableProof, Vec<u8>)>,
    eval_line: Vec<Scalar>,
}

type ProposeExtraction = (RSDecoderData, Vec<Scalar>, Vec<(MProof, Vec<u8>)>);
impl ProposeMessage {
    pub fn get_transcript(
        datas: RSDecoderData,
        shares_and_proofs: Vec<(MProof, Vec<u8>)>,
        eval_line: Vec<Scalar>,
    ) -> Vec<u8> {
        let tr = Self {
            datas,
            eval_line,
            shares_and_proofs: shares_and_proofs
                .into_iter()
                .map(|(p, s)| (SerializableProof::from_proof(&p), s))
                .collect(),
        };
        enc!(SecureMsgDis, SecureMsgDisCommand::Propose, tr)
    }

    pub fn extract(self) -> ProposeExtraction {
        (
            self.datas,
            self.eval_line,
            self.shares_and_proofs
                .into_iter()
                .map(|(p, s)| (p.to_proof(), s))
                .collect(),
        )
    }
}
