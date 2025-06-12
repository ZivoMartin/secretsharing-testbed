use crate::disperse_retrieve::messages::{Echo, Propose, Ready};
use crate::{
    crypto::{Share, Sign},
    haven::crypto::{EchoMessage, SendMessage},
    hbavss::{HbAvssAssist, HbAvssComplaint},
    secure_message_dist::{ForwardMessage, ForwardTag, SmdMemory},
    system::node_sender::ChannelId,
};
use global_lib::{messages::NameSpace, Wrapped};
use notifier_hub::closable_trait::ClosableMessage;
use sendable_proc_macros::Sendable;

type Bytes = Vec<u8>;
type WrappedSmdMem = Wrapped<SmdMemory>;

#[derive(Clone, Sendable)]
pub enum NodeMessage {
    ShareReceived,
    BeaconSender(Bytes),
    SMDSender(Bytes),
    SMDForwardRequest(ForwardTag),
    SMDForwardLightWeightComplaint(WrappedSmdMem, ForwardMessage),
    SMDForwardLightWeightAssist(WrappedSmdMem, ForwardMessage),
    SMDForwardLightWeightReport(WrappedSmdMem, ForwardMessage),
    SMDOutput(Bytes),
    BroadcastSender(Bytes),
    BroadcastAvssSimpl(Bytes),
    BroadcastBingo(Bytes),
    BroadcastLightWeight(Bytes),
    BroadcastBadger(Bytes),
    BroadcastHbAvss(Bytes),
    AvssSimplSender(Bytes),
    AvssSimplDealerMessage(u16, Sign),
    AvssSimplExtShare(Share),
    BingoSender(Bytes),
    BingoRow(Share),
    BingoCol(Share),
    BingoDone,
    BingoBroadcastDoneRequest(Vec<Share>),
    BingoReconstructShare(Share),
    LightWeightSender(Bytes),
    LightWeightEndOfProcessing,
    OneSidedVoteSender(Bytes),
    OneSidedVoteBroadcastVoteRequest,
    OneSidedVoteBroadcastOkRequest,
    OneSidedVoteOutput,
    DispRetSender(Bytes),
    DispRetAddShare(Echo),
    DispRetOutputReq,
    DispRetEcho(Echo),
    DispRetReady(Ready),
    DispRetDisperseComplete,
    DispRetRetrieveRequest(usize),
    DispRetPropose(Propose),
    DispRetRetrieveOutput(Bytes),
    BadgerSender(Bytes),
    BadgerReconstructShare(Share),
    HbAvssSender(Bytes),
    HbAvssEndOfProcessing,
    HbAvssComplaint(HbAvssComplaint),
    HbAvssAssist(HbAvssAssist),
    HavenSender(Bytes),
    HavenSend(SendMessage),
    HavenEcho(EchoMessage),
    HavenReady(Vec<u8>),
    Close,
}

impl ClosableMessage for NodeMessage {
    fn get_close_message() -> Self {
        Self::Close
    }
}

pub fn namespace_to_channel_id(namespace: NameSpace) -> ChannelId {
    match namespace {
        NameSpace::Haven => NodeMessage::HavenSenderConst,
        NameSpace::Broadcast => NodeMessage::BroadcastSenderConst,
        NameSpace::SecureMsgDis => NodeMessage::SMDSenderConst,
        NameSpace::AvssSimpl => NodeMessage::AvssSimplSenderConst,
        NameSpace::Bingo => NodeMessage::BingoSenderConst,
        NameSpace::LightWeight => NodeMessage::LightWeightSenderConst,
        NameSpace::Badger => NodeMessage::BadgerSenderConst,
        NameSpace::HbAvss => NodeMessage::HbAvssSenderConst,
        NameSpace::OneSidedVote => NodeMessage::OneSidedVoteSenderConst,
        NameSpace::DisperseRetrieve => NodeMessage::DispRetSenderConst,
        NameSpace::Heart => panic!("Private namespace"),
    }
}
