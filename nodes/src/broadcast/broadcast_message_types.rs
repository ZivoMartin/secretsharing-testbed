use crate::{
    crypto::data_structures::reed_solomon_code::RSDecoderData, node::node_message::NodeMessage,
};
use global_lib::as_number;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Transcript {
    pub i: u16,
    pub kind: BroadcastMessageType,
    pub hash: Vec<u8>,
    pub datas: RSDecoderData,
    pub share: Vec<u8>,
}

pub struct ReadyTranscript {}

as_number!(
    u8,
    enum BroadcastMessageType {
        AvssSimpl,
        Bingo,
        Badger,
        LightWeight,
        HbAvss,
    },
    derive(Hash, Copy, Eq, PartialEq, Clone, Serialize, Deserialize)
);

impl BroadcastMessageType {
    pub fn get_node_message(self, message: Vec<u8>) -> NodeMessage {
        match self {
            Self::AvssSimpl => NodeMessage::BroadcastAvssSimpl(message),
            Self::Bingo => NodeMessage::BroadcastBingo(message),
            Self::Badger => NodeMessage::BroadcastBadger(message),
            Self::LightWeight => NodeMessage::BroadcastLightWeight(message),
            Self::HbAvss => NodeMessage::BroadcastHbAvss(message),
        }
    }
}
