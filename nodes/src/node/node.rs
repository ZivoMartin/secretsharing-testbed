use super::{
    configuration::Configuration,
    node_message::{namespace_to_channel_id, NodeMessage},
    node_process_input::NodeProcessInput,
    node_process_output::NodeProcessOutput,
};

use crate::{
    avss_simpl::{
        messages_receiver::{avss_simpl_share, listen_at as avss_simpl_listen},
        reconstruct::reconstruct as avss_simpl_reconstruct,
    },
    badger::{
        messages_receiver::{badger_share, listen_at as badger_listen},
        reconstruct::reconstruct as badger_reconstruct,
    },
    bingo::{
        messages_receiver::{bingo_share, listen_at as bingo_listen},
        reconstruct::reconstruct as bingo_reconstruct,
    },
    broadcast::{
        broadcast_message_types::BroadcastMessageType, listener::listen as broadcast_listen,
    },
    crypto::{
        crypto_lib::evaluation_domain::BatchEvaluationDomain,
        crypto_set::CryptoSet,
        data_structures::{
            keypair::{KeyPair, PublicKey},
            share::Share,
        },
        Commitment, Secret, Sign,
    },
    disperse_retrieve::{disperse_retrieve_listener, get_disperse_messages},
    haven::messages_receiver::{haven_share, listen_at as haven_listen},
    hbavss::{hbavss_listen, hbavss_share},
    lightweight::{lightweight_listen, lightweight_share},
    one_sided_vote::one_sided_vote_listen,
    secure_message_dist::{get_secure_message_dis_transcripts, listen as smd_listen, ForwardTag},
    system::{message_interface::SendableMessage, node_sender::ChannelId, summaries::Summaries},
};
use aptos_crypto::bls12381::PublicKey as SigningPublicKey;
use blstrs::Scalar;
use blsttc::{Ciphertext, SecretKey};
use std::sync::Arc;

use std::{collections::HashMap, fs::File, io::Write, time::Instant};

use notifier_hub::{
    notifier::{ChannelState, MessageReceiver as Receiver, NotifierHub},
    writing_handler::WritingHandler,
};
type WritingFuture = WritingHandler<NodeMessage>;

use global_lib::{
    config_treatment::result_fields::ResultDuration,
    dec, enc,
    messages::{Algo, BroadcastCommand, NameSpace},
    network::Network,
    process::ProcessTrait,
    settings::VERBOSE,
    task_pool::task::TaskInterface,
    wrap, NodeId, OpId, Step, Wrapped,
};
use tokio::{
    spawn,
    sync::mpsc::{channel, Sender},
    task::JoinHandle,
};

pub type Handler = JoinHandle<()>;
pub type Message = Vec<u8>;

#[macro_export]
macro_rules! log {
    ($name:ident, $msg:expr $(, $($args:expr),*)?) => {
        if global_lib::settings::VERBOSE {
            let mut node = $name.lock().await;
            let f = node.log.as_mut().unwrap();
            if let Err(e) = writeln!(&*f, "{}", format!($msg $(, $($args),*)?)) {
                println!("Failed to log \"{:?}\" because of {e:?}", $msg)
            }
        }
    };
}

pub struct Node {
    pub log: Option<File>,
    config: Configuration,            // Configuration of the process
    network: Network,                 // Network of the process (gift at init)
    public_keys: Arc<Vec<PublicKey>>, // The set of public key (gift at init)
    senders: NotifierHub<NodeMessage, ChannelId>, // Sender network
    shares: CryptoSet,                // A container for cryptographics objects
    keypair: KeyPair,                 // The keypair of the node, contains all his private keys
    timer: Instant,                   // Timer used to compute the lifetime of the process
    im_done: bool,                    // Assure that the node will output only once
    result_sender: TaskInterface<NodeProcessOutput>, // Result sender of the process pool
    summaries: Option<Summaries>,
    log_file: Option<File>,
    handlers: Option<Vec<Handler>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        if VERBOSE && self.im_done() {
            std::fs::remove_file(format!("../logs/node_{}_{}", self.index(), self.op_id()))
                .unwrap();
        }
    }
}

impl ProcessTrait<NodeProcessInput, Message, NodeProcessOutput> for Node {
    fn begin(
        input: NodeProcessInput,
        result_sender: TaskInterface<NodeProcessOutput>,
    ) -> Sender<Message> {
        let (message_sender, message_receiver) = channel(1000);
        let step = input.fields.step();
        let algo = input.fields.algo();
        let node = Node::new(input, result_sender);
        spawn(async move {
            Self::start_listener(node.clone(), algo, message_receiver).await;
            match step {
                Step::Sharing => Self::share(node, algo).await,
                Step::Reconstruct => Self::reconstruct(node, algo).await,
            }
        });
        message_sender
    }
}

impl Node {
    pub fn new(
        input: NodeProcessInput,
        result_sender: TaskInterface<NodeProcessOutput>,
    ) -> Wrapped<Node> {
        let NodeProcessInput {
            id,
            index,
            mut fields,
            network,
            shares,
            keypair,
            public_keys,
            dealer,
            base,
        } = input;
        let log_file = None;

        let mut summ = Summaries::new(index);
        summ.set_n(fields.n() as usize);
        wrap!(Node {
            log: if VERBOSE {
                Some(File::create(format!("../logs/node_{index}_{id}")).unwrap())
            } else {
                None
            },
            config: Configuration::from_fields(&mut fields, base, index, id, dealer),
            timer: Instant::now(),
            handlers: Some(Vec::new()),
            network,
            summaries: Some(summ),
            result_sender,
            senders: NotifierHub::new(),
            shares,
            keypair,
            public_keys: Arc::new(public_keys),
            im_done: false,
            log_file
        })
    }

    async fn share(node: Wrapped<Node>, algo: Algo) {
        log!(node, "Sharing with {algo}");
        node.clone().lock().await.push_handler(spawn(async move {
            match algo {
                Algo::Haven => haven_share(node).await,
                Algo::Bingo => bingo_share(node).await,
                Algo::AvssSimpl | Algo::DualAvssSimpl => avss_simpl_share(node).await,
                Algo::LightWeight => lightweight_share(node).await,
                Algo::Badger => badger_share(node).await,
                Algo::HbAvss => hbavss_share(node).await,
            }
        }));
    }

    async fn reconstruct(node: Wrapped<Node>, algo: Algo) {
        log!(node, "Reconstructing with {algo}");
        match algo {
            Algo::AvssSimpl => avss_simpl_reconstruct(node).await,
            Algo::Bingo => bingo_reconstruct(node).await,
            Algo::Badger => badger_reconstruct(node).await,
            _ => {
                panic!("can't reconstruct with {algo}.")
            }
        }
    }

    fn node_message_from_namespace(namespace: NameSpace, bytes_message: Vec<u8>) -> NodeMessage {
        match namespace {
            NameSpace::AvssSimpl => NodeMessage::AvssSimplSender(bytes_message),
            NameSpace::Haven => NodeMessage::HavenSender(bytes_message),
            NameSpace::Bingo => NodeMessage::BingoSender(bytes_message),
            NameSpace::LightWeight => NodeMessage::LightWeightSender(bytes_message),
            NameSpace::Badger => NodeMessage::BadgerSender(bytes_message),
            NameSpace::HbAvss => NodeMessage::HbAvssSender(bytes_message),
            NameSpace::Broadcast => NodeMessage::BroadcastSender(bytes_message),
            NameSpace::SecureMsgDis => NodeMessage::SMDSender(bytes_message),
            NameSpace::OneSidedVote => NodeMessage::OneSidedVoteSender(bytes_message),
            NameSpace::DisperseRetrieve => NodeMessage::DispRetSender(bytes_message),
            NameSpace::Heart => panic!("I can't receiv a heart message"),
        }
    }

    pub async fn listen_at(
        node: Wrapped<Node>,
        mut receiver: tokio::sync::mpsc::Receiver<Message>,
    ) {
        while let Some(mut bytes_message) = receiver.recv().await {
            let msg =
                Self::node_message_from_namespace(bytes_message.remove(0).into(), bytes_message);
            Self::wait_and_send(&node, msg).await;
        }
        log!(node, "Exiting listen_at");
    }

    pub fn get_comm(&self) -> &Commitment {
        self.set().get_comm()
    }

    pub fn op_id(&self) -> OpId {
        self.config.id()
    }

    pub fn subscribe(&mut self, id: ChannelId) -> Receiver<NodeMessage> {
        self.senders.subscribe(&id, 100)
    }

    pub fn subscribe_multiple(&mut self, ids: &[ChannelId]) -> Receiver<NodeMessage> {
        self.senders.subscribe_multiple(ids, 100)
    }

    pub async fn wait_and_send(node: &Wrapped<Node>, msg: NodeMessage) -> WritingFuture {
        let id = msg.to_str();
        Self::wait_for_channel(node, id).await.unwrap();
        node.lock().await.send_message(msg).await
    }

    pub async fn try_wait_and_send(
        node: &Wrapped<Self>,
        msg: NodeMessage,
    ) -> Result<WritingFuture, String> {
        let id = msg.to_str();
        match Self::wait_for_channel(node, id).await {
            Some(_) => node.lock().await.try_send_message(msg).await,
            None => Err(format!("Failed to wait for channel {id}")),
        }
    }

    pub fn get_waiter(&mut self, channel: &'static str) -> Option<Receiver<()>> {
        match self.senders.channel_state(&channel) {
            ChannelState::Uninitialised => Some(self.senders.get_creation_waiter(&channel)),
            _ => None,
        }
    }

    pub async fn wait_for_channel(node: &Wrapped<Self>, channel: &'static str) -> Option<()> {
        let receiver = node.lock().await.get_waiter(channel);
        if let Some(mut receiver) = receiver {
            receiver.recv().await
        } else {
            Some(())
        }
    }

    pub fn im_done(&self) -> bool {
        self.im_done
    }

    pub fn push_handler(&mut self, handler: Handler) {
        match &mut self.handlers {
            Some(h) => h.push(handler),
            None => eprintln!("Pushing a handler but the node has output"),
        }
    }

    async fn start_listener_from_namespace(node: Wrapped<Node>, namespace: NameSpace) -> Handler {
        spawn(async move {
            match namespace {
                NameSpace::Broadcast => broadcast_listen(node).await,
                NameSpace::Haven => haven_listen(node).await,
                NameSpace::SecureMsgDis => smd_listen(node).await,
                NameSpace::AvssSimpl => avss_simpl_listen(node).await,
                NameSpace::Bingo => bingo_listen(node).await,
                NameSpace::LightWeight => lightweight_listen(node).await,
                NameSpace::Badger => badger_listen(node).await,
                NameSpace::HbAvss => hbavss_listen(node).await,
                NameSpace::OneSidedVote => one_sided_vote_listen(node).await,
                NameSpace::DisperseRetrieve => disperse_retrieve_listener(node).await,
                NameSpace::Heart => panic!("Private namespace"),
            }
        })
    }

    async fn start_listener(
        node: Wrapped<Node>,
        algo: Algo,
        message_receiver: tokio::sync::mpsc::Receiver<Message>,
    ) {
        let mut to_init = algo.get_subprotocols();
        to_init.push(NameSpace::from(algo));
        for namespace in to_init {
            let handler = Self::start_listener_from_namespace(node.clone(), namespace).await;
            node.lock().await.push_handler(handler);
            Self::wait_for_channel(&node, namespace_to_channel_id(namespace)).await;
        }
        spawn(async move {
            Self::listen_at(node, message_receiver).await;
        });
    }

    pub fn kill_channel(&mut self, channel: ChannelId) {
        self.subscribe(channel);
    }

    pub async fn try_send_message(&self, message: NodeMessage) -> Result<WritingFuture, String> {
        if self.im_done() {
            Ok(WritingFuture::empty())
        } else {
            let id = message.to_str();
            match self.senders.clone_send(message, &id) {
                Ok(w) => Ok(w),
                Err(_) => Err("Failed to send".to_string()),
            }
        }
    }

    pub async fn send_message(&self, message: NodeMessage) -> WritingFuture {
        if self.im_done() {
            WritingFuture::empty()
        } else {
            let id = message.to_str();
            match self.senders.clone_send(message, &id) {
                Ok(w) => w,
                Err(_) => panic!(),
            }
        }
    }

    pub fn channel_is_setup(&self, id: ChannelId) -> bool {
        self.senders.channel_state(&id) == ChannelState::Running
    }

    pub fn throw_my_share(&mut self) -> Share {
        let index = self.index();
        self.set_mut().throw(index)
    }

    pub async fn save_share(&mut self, share: Share) {
        self.set_mut().new_share(share);
        if self.channel_is_setup(NodeMessage::ShareReceivedConst) {
            if self
                .send_message(NodeMessage::ShareReceived)
                .await
                .wait(None)
                .await
                .is_err()
            {
                panic!()
            }
        }
    }

    pub async fn save_shares(&mut self, shares: Vec<Share>) {
        self.set_mut().set_shares(shares);
        if self.channel_is_setup(NodeMessage::ShareReceivedConst) {
            self.send_message(NodeMessage::ShareReceived).await;
        }
    }

    pub fn fake_sign(&self) -> Sign {
        self.keypair.fake_sign(&enc!(self.get_comm()))
    }

    pub fn sign(&self) -> Sign {
        self.keypair.sign(&enc!(self.get_comm()))
    }

    pub fn shares_vec(&self) -> Vec<Share> {
        let mut shares = self
            .set()
            .set()
            .iter()
            .map(|(_, s)| s.clone())
            .collect::<Vec<_>>();
        shares.sort();
        shares
    }

    pub fn shares_map(&self) -> &HashMap<u16, Share> {
        self.set().set()
    }

    pub fn has_comm(&self) -> bool {
        self.set().has_comm()
    }

    pub fn has_share(&self) -> bool {
        self.set().contains(self.index())
    }

    pub fn my_share(&self) -> &Share {
        self.set().get(self.index())
    }

    pub fn set(&self) -> &CryptoSet {
        &self.shares
    }

    pub fn set_mut(&mut self) -> &mut CryptoSet {
        &mut self.shares
    }

    pub fn im_dealer(&self) -> bool {
        self.index() == self.config().dealer()
    }

    pub fn step(&self) -> Step {
        self.config.step()
    }

    fn get_result(&mut self) -> ResultDuration {
        self.timer.elapsed().as_millis() as ResultDuration
    }

    pub fn output(node: Wrapped<Self>) {
        spawn(async move {
            log!(node, "Outputing !");
            let (result, set, handlers) = {
                let mut node = node.lock().await;
                // assert!(node.has_share() && node.has_comm());
                node.im_done = true;
                let handlers = node
                    .handlers
                    .take()
                    .unwrap()
                    .drain(..)
                    .collect::<Vec<Handler>>();
                let result = node.get_result();
                let set = match node.step() {
                    Step::Sharing => Some(node.shares.extract()),
                    Step::Reconstruct => None,
                };
                node.senders.shutdown_all_clone();
                (result, set, handlers)
            };
            log!(node, "Waiting for handlers..");
            for h in handlers {
                h.await.unwrap();
            }
            log!(node, "Preparing output notif..");
            let output = {
                let mut node = node.lock().await;
                let summaries = node.summaries.take().unwrap();
                let output = (node.op_id(), NodeProcessOutput::new(result, summaries, set));
                output.1
            };
            log!(node, "Sending output notif");
            node.lock()
                .await
                .result_sender
                .output(output)
                .await
                .unwrap();
            log!(
                node,
                "Successfully sent output notif, {}",
                Arc::strong_count(&node)
            );
        });
    }

    pub fn get_all_pkey(&self) -> Arc<Vec<PublicKey>> {
        self.public_keys.clone()
    }

    pub fn my_decrypt_skey(&self) -> &Scalar {
        self.keypair.private_decrypt_key()
    }

    pub fn my_blstt_skey(&self) -> &SecretKey {
        self.keypair.private_blstt_decrypt_key()
    }

    pub fn public_signing_key(&self) -> &SigningPublicKey {
        self.keypair.public_signing_key()
    }

    pub fn get_specific_key(&self, index: u16) -> &PublicKey {
        &self.public_keys[index as usize]
    }

    pub fn get_network_mut(&mut self) -> &mut Network {
        &mut self.network
    }

    pub fn get_network(&self) -> &Network {
        &self.network
    }

    pub fn n(&self) -> u16 {
        self.config.n()
    }

    pub fn t(&self) -> u16 {
        self.config.t()
    }

    pub fn l(&self) -> u16 {
        self.config.l()
    }

    pub fn dealer_corruption(&self) -> u16 {
        self.config().dealer_corruption()
    }

    pub fn batch_size(&self) -> usize {
        self.config().batch_size()
    }

    pub fn is_byz(&self) -> bool {
        self.config.is_byz()
    }

    pub fn is_dealer_corrupted(&self) -> bool {
        self.config().dealer_corruption() > 0
    }

    pub fn config(&self) -> &Configuration {
        &self.config
    }

    pub fn uindex(&self) -> usize {
        self.index() as usize
    }

    pub fn index(&self) -> u16 {
        self.config.index()
    }

    pub fn get_secrets(&self) -> &Option<Vec<Secret>> {
        self.set().get_secrets()
    }

    pub fn set_secrets(&mut self, secrets: Vec<Secret>) {
        self.set_mut().set_secrets(secrets);
    }

    pub fn reset_timer(&mut self) {
        self.timer = Instant::now();
    }

    pub fn set_comm(&mut self, comm: Commitment) {
        self.shares.set_comm(comm);
    }

    pub fn dom(&self) -> &BatchEvaluationDomain {
        self.config().get_batch_evaluation_domain()
    }

    pub fn log(&mut self, s: &str) {
        let id = self.config.id();
        let index = self.index();
        if let Some(file) = &mut self.log_file {
            file.write_all(&format!("P {} Node {}: {s}\n", id, index).into_bytes())
                .expect("Failed to log");
        }
    }

    pub fn give_contact(&mut self, i: usize, msg: Vec<u8>) {
        self.summaries.as_mut().unwrap().new_message_sent(i);
        let id = self.op_id();
        let index = self.index();
        self.get_network().give_message(i, msg, id, index);
    }

    pub fn contact(&mut self, i: usize, msg: Arc<Vec<u8>>) {
        self.summaries.as_mut().unwrap().new_message_sent(i);
        let id = self.op_id();
        let index = self.index();
        self.get_network().message(i, msg, id, index);
    }

    // pub fn contact_multiple(&mut self, i: usize, messages: Vec<&[u8]>) {
    //     self.summaries.as_mut().unwrap().new_message_sent(i);
    //     let id = self.op_id();
    //     let index = self.index();
    //     self.get_network().message_multiple(i, messages, id, index);
    // }

    pub async fn contact_dealer(&mut self, msg: Vec<u8>) {
        let dealer = self.config().dealer() as usize;
        self.contact(dealer, Arc::new(msg))
    }

    pub async fn disperse(&mut self, messages: Vec<Vec<u8>>) {
        let messages = get_disperse_messages(messages, self.n() as usize, self.t() as usize);
        let id = self.index();
        let mut my_message = Vec::new();
        for (i, msg) in (0..self.n() - self.dealer_corruption()).zip(messages.into_iter()) {
            if i != id {
                self.contact(i as usize, Arc::new(msg))
            } else {
                my_message = msg
            }
        }
        if !my_message.is_empty() {
            let msg = Self::node_message_from_namespace(my_message.remove(0).into(), my_message);
            self.send_message(msg).await;
        }
    }

    pub async fn retrieve(node: &Wrapped<Self>, i: usize, skey: Option<SecretKey>) -> Vec<u8> {
        let req = NodeMessage::DispRetRetrieveRequest(i);
        node.lock().await.send_message(req).await;
        let channel = NodeMessage::DispRetRetrieveOutputConst;
        let mut receiver = node.lock().await.subscribe(channel);
        if let NodeMessage::DispRetRetrieveOutput(res) = receiver.recv().await.unwrap() {
            if let Some(skey) = skey {
                let cpt: Ciphertext = dec!(res);
                skey.decrypt(&cpt).unwrap()
            } else {
                res
            }
        } else {
            panic!()
        }
    }

    pub async fn distribute(&mut self, messages: Vec<Vec<u8>>) {
        let messages =
            get_secure_message_dis_transcripts(messages, self.n() as usize, self.t() as usize + 1);
        let id = self.index();
        let mut my_message = Vec::new();
        for (i, msg) in (0..self.n() - self.dealer_corruption()).zip(messages.into_iter()) {
            if i != id {
                self.contact(i as usize, Arc::new(msg))
            } else {
                my_message = msg
            }
        }
        if !my_message.is_empty() {
            let msg = Self::node_message_from_namespace(my_message.remove(0).into(), my_message);
            self.send_message(msg).await;
        }
    }

    pub async fn forward(node: &Wrapped<Node>, tag: ForwardTag) {
        if Self::wait_and_send(node, NodeMessage::SMDForwardRequest(tag))
            .await
            .wait(None)
            .await
            .is_err()
        {
            panic!()
        }
    }

    pub async fn broadcast(&mut self, msg: Vec<u8>, with_me: bool) {
        let mut to_contact: Vec<usize> = (0..self.n() as usize).collect();
        to_contact.remove(self.index() as usize);
        self.broadcast_specific_network_part(msg, with_me, to_contact)
            .await
    }

    async fn broadcast_specific_network_part(
        &mut self,
        mut msg: Vec<u8>,
        with_me: bool,
        to_contact: Vec<usize>,
    ) {
        let id = self.op_id();
        let index = self.index();

        for i in &to_contact {
            self.summaries.as_mut().unwrap().new_message_sent(*i)
        }

        self.get_network_mut()
            .broadcast(msg.clone(), id, Some(to_contact), index);
        if with_me {
            let msg = Self::node_message_from_namespace(msg.remove(0).into(), msg);
            self.send_message(msg).await;
        }
    }

    pub async fn reliable_broadcast(&mut self, kind: BroadcastMessageType, mut message: Vec<u8>) {
        let mut b_message = vec![
            NameSpace::Broadcast.into(),
            BroadcastCommand::Propose.into(),
            kind.into(),
        ];
        b_message.append(&mut message);
        let to_contact: Vec<usize> =
            (0..(self.n() - self.dealer_corruption()) as usize).collect::<Vec<usize>>();
        self.broadcast_specific_network_part(b_message, false, to_contact)
            .await;
    }

    pub async fn encrypt_message(&self, msg: Vec<u8>, pkey: NodeId) -> Vec<u8> {
        enc!(self.get_specific_key(pkey).blstt_encrypt(msg))
    }
}
