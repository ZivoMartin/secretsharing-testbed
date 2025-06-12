use super::{
    heart_message::{HeartMessage, NewMessage},
    message_interface::SendableMessage,
    node_sender::ChannelId,
    summaries::{Summaries, SummaryMessage},
};
use crate::{
    break_if_over,
    crypto::{
        crypto_set::{CryptoSet, CryptoSetIdentity},
        data_structures::{
            keypair::{KeyPair, PublicKey},
            Base,
        },
    },
    node::{
        node::{Message, Node},
        node_process_input::NodeProcessInput,
        node_process_output::NodeProcessOutput,
    },
    panic_if_over,
};
use global_lib::{
    async_private_message,
    config_treatment::fields::Fields,
    dec, enc, explicit_log,
    ip_addr::IpV4,
    log,
    messages::{InterfaceCode, NameSpace, NodeCommand},
    network::Network,
    process_pool::{PoolProcessEnded, ProcessPool},
    select,
    settings::TIMEOUT,
    wrap, NodeId, OpId, Step, Wrapped, ANONYMOUS,
};
use rand::thread_rng;
use std::fs::File;
use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
    ops::AddAssign,
    process::exit,
    sync::Arc,
};
use tokio::{pin, select as tk_select, spawn};

use notifier_hub::{
    notifier::{ChannelState, MessageReceiver as Receiver, NotifierHub},
    writing_handler::WritingHandler,
};

type WritingFuture = WritingHandler<HeartMessage>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupData {
    index: u16,
    base: Base,
}

type ShareMap = HashMap<CryptoSetIdentity, CryptoSet>;

#[derive(Clone)]
pub struct NodesHeart {
    log: Wrapped<File>,
    pool: ProcessPool<Message, NodeProcessOutput>,
    my_ip: IpV4,
    interface_ip: IpV4,
    senders: Wrapped<NotifierHub<HeartMessage, ChannelId>>,
    network: Wrapped<Network>,
    public_keys: Wrapped<Vec<PublicKey>>,
    keypair: Wrapped<Option<KeyPair>>,
    index: Option<u16>,
    shares_map: Wrapped<ShareMap>,
    base: Option<Base>,
}

impl NodesHeart {
    pub async fn new(interface_ip: IpV4, my_ip: IpV4) -> Self {
        let heart = NodesHeart {
            log: wrap!(File::create(&format!("../logs/node_{my_ip}")).unwrap()),
            pool: ProcessPool::default(),
            my_ip,
            interface_ip,
            network: wrap!(Network::new()),
            public_keys: wrap!(Vec::new()),
            senders: wrap!(NotifierHub::new()),
            keypair: wrap!(None),
            shares_map: wrap!(HashMap::new()),
            base: None,
            index: None,
        };
        heart.clone().listen_for_results();
        heart.clone().key_waiter();
        heart.clone().message_listener();
        log!(heart, "Just started all the listeners");
        heart
    }

    fn pool_result_redirecter(
        self,
        mut pool_receiver: tokio::sync::mpsc::Receiver<PoolProcessEnded<NodeProcessOutput>>,
    ) {
        spawn(async move {
            while let Some(output) = pool_receiver.recv().await {
                log!(self, "New output: {output:?}");
                let msg = HeartMessage::PoolOutput(output);
                self.send_message(msg).await;
                log!(self, "Output message sent !");
            }
        });
    }

    async fn handle_node_output(
        &self,
        output: Arc<NodeProcessOutput>,
        id: OpId,
        set_already_saved: &mut HashSet<CryptoSetIdentity>,
        current_summaries: &mut Summaries,
    ) {
        let NodeProcessOutput {
            share_set,
            result,
            summaries,
        } = (*output).clone();
        if let Some(share_set) = share_set {
            let ident = share_set.identity();
            if !set_already_saved.contains(&ident) {
                self.save_share_set(share_set).await;
                set_already_saved.insert(ident);
            }
        }
        current_summaries.add_assign(summaries);

        let mut msg = vec![InterfaceCode::Output.into()];
        enc!(result, msg);
        self.contact_interface(msg, id);
    }

    async fn send_summaries(&self, summ: &mut Summaries) {
        log!(self, "Sending summaries: {summ:?}");
        let summaries = summ.get_messages();
        let network = self.network.lock().await;
        for (i, s) in summaries {
            let msg = Arc::new(enc!(Heart, NodeCommand::Summary, s));
            network.message(i, msg, 0, ANONYMOUS);
        }
        summ.clear();
    }

    fn listen_for_results(self) {
        spawn(async move {
            let channels = [
                HeartMessage::PoolOutputConst,
                HeartMessage::GiveSummConst,
                HeartMessage::SetupOverConst,
                HeartMessage::EmitNConst,
            ];

            let mut receiver = self.subscribe_multiple(&channels).await;

            let result_receiver = self.pool.new_result_redirection().await;
            self.clone().pool_result_redirecter(result_receiver);

            let (mut set_already_saved, mut current_summaries, mut started, mut process_counter) =
                (HashSet::new(), Summaries::new(0), None, 0);

            loop {
                let msg = break_if_over!(receiver);
                match msg {
                    HeartMessage::PoolOutput(PoolProcessEnded { output, id, .. }) => {
                        self.handle_node_output(
                            output,
                            id,
                            &mut set_already_saved,
                            &mut current_summaries,
                        )
                        .await;
                        process_counter += 1;
                        if Some(process_counter) == started {
                            log!(self, "Result Listener: Sending summaries");
                            self.send_summaries(&mut current_summaries).await;
                            started = None;
                            process_counter = 0;
                        } else {
                            log!(self, "Result Listener: Received an output ! We have to wait for {started:?}, we have {process_counter}");
                        }
                    }
                    HeartMessage::EmitN(n) => {
                        log!(self, "Result Listener: Juste got n for summaries, {n}");
                        current_summaries.set_n(n);
                    }
                    HeartMessage::GiveSumm(started_received) => {
                        log!(self, "Result Listener: Give Summ order received, to wait = {started_received}. I have {process_counter}");
                        if process_counter == started_received {
                            self.send_summaries(&mut current_summaries).await;
                            process_counter = 0;
                        } else {
                            started = Some(started_received)
                        }
                    }
                    HeartMessage::SetupOver(d) => {
                        log!(
                            self,
                            "Result Listener: Setup is over, setting the index, {}",
                            d.index
                        );
                        current_summaries.set_index(d.index);
                    }
                    _ => panic!("Unexpected message"),
                }
            }
        });
    }

    pub async fn new_message(&self, bytes: Vec<u8>, sender: NodeId, id: OpId) {
        self.send_message(HeartMessage::MessageSender(NewMessage {
            bytes,
            sender,
            id,
        }))
        .await;
    }

    async fn handle_message(self, mut bytes: Vec<u8>, id: OpId) {
        match bytes[0].into() {
            NameSpace::Heart => {
                bytes.remove(0);
                self.heart_command(bytes, id).await
            }
            _ => self.send_bytes(bytes, id),
        }
    }

    pub fn message_listener(mut self) {
        spawn(async move {
            let channels = [
                HeartMessage::MessageSenderConst,
                HeartMessage::EmitSummConst,
                HeartMessage::SetupOverConst,
                HeartMessage::EmitNConst,
                HeartMessage::ForceClearSummConst,
            ];
            let mut receiver = self.subscribe_multiple(&channels).await;
            let mut summaries = Summaries::new(0);
            fn clear_summ(heart: &NodesHeart, summ: &mut Summaries) {
                summ.clear();
                let msg = HeartMessage::NetworkCleared;
                let cloned = heart.clone();
                spawn(async move { cloned.wait_and_send(msg).await.wait(None).await.unwrap() });
            }

            fn check_summ(heart: &NodesHeart, summ: &mut Summaries) {
                if summ.is_done() {
                    clear_summ(heart, summ)
                }
            }
            loop {
                let msg = panic_if_over!(receiver);
                match msg {
                    HeartMessage::MessageSender(NewMessage { bytes, sender, id }) => {
                        if sender != ANONYMOUS {
                            log!(
                                self,
                                "Message Listener: New message received from {sender} !"
                            );
                            summaries.new_message_received(sender as usize);
                            check_summ(&self, &mut summaries);
                        }
                        let cloned_self = self.clone();
                        tokio::spawn(async move { cloned_self.handle_message(bytes, id).await });
                    }
                    HeartMessage::EmitSumm(s) => {
                        log!(self, "Message Listener: New summary received ! {s:?} !");
                        summaries += s;
                        check_summ(&self, &mut summaries)
                    }
                    HeartMessage::ForceClearSumm => {
                        log!(
                            self,
                            "Message Listener: Just received ForceClearSumm, clearing and sending"
                        );
                        clear_summ(&self, &mut summaries);
                    }
                    HeartMessage::EmitN(n) => {
                        log!(self, "Message Listener: Just received n, {n}");
                        summaries.set_n(n);
                    }
                    HeartMessage::SetupOver(d) => {
                        log!(self, "Message Listener: Just received index, {}", d.index);
                        summaries.set_index(d.index);
                        self.save_setup(d);
                    }
                    _ => panic!("Unexpected message"),
                }
            }
        });
    }

    pub async fn heart_command(self, bytes_message: Vec<u8>, id: OpId) {
        log!(
            self,
            "New command: {:?}",
            NodeCommand::from(bytes_message[0])
        );
        select!(
            self_select, NodeCommand, bytes_message, self,
            Setup => setup,
            Kill => kill_myself,
            Key => new_key,
            Process => new_process id,
            Clean => clean,
            Summary => new_summ
        );
    }

    fn send_bytes(self, bytes: Vec<u8>, id: OpId) {
        spawn(async move {
            let _ = self.pool.wait_and_send(id, bytes).await;
        });
    }

    async fn new_process(&self, bytes: &[u8], id: OpId) {
        log!(self, "New process: {id}");
        let fields: Fields = dec!(bytes, Fields);
        let set_identity = (fields.n(), fields.t(), fields.algo());
        let share_set = match fields.step() {
            Step::Sharing => CryptoSet::new(set_identity),
            Step::Reconstruct => self.get_share_set(set_identity).await,
        };

        let n = fields.n();
        self.send_message(HeartMessage::EmitN(n as usize)).await;

        let network = {
            let mut network = self.network.lock().await;
            network.switch_on(global_lib::KindEvaluation::Debit);
            network.adjust(n as usize).await;
            network.extract_subnetwork(n as usize)
        };
        let public_keys = self.public_keys.lock().await[..n as usize].to_vec();

        let input = NodeProcessInput::new(
            fields,
            id,
            self.index(),
            self.keypair.lock().await.as_ref().unwrap().clone(),
            network,
            public_keys,
            share_set,
            id as u16 % n,
            *self.base.as_ref().unwrap(),
        );
        self.pool
            .new_task::<Node, NodeProcessInput>(id, input)
            .await
            .unwrap();
    }

    pub async fn subscribe(&self, id: ChannelId) -> Receiver<HeartMessage> {
        self.senders.lock().await.subscribe(&id, 100)
    }

    pub async fn subscribe_multiple(&self, ids: &[ChannelId]) -> Receiver<HeartMessage> {
        self.senders.lock().await.subscribe_multiple(ids, 100)
    }

    pub async fn channel_is_setup(&self, id: ChannelId) -> bool {
        self.senders.lock().await.channel_state(&id) == ChannelState::Running
    }

    pub async fn wait_and_send(&self, msg: HeartMessage) -> WritingFuture {
        let id = msg.to_str();
        self.wait_for_channel(id).await.unwrap();
        self.send_message(msg).await
    }

    pub async fn try_wait_and_send(&self, msg: HeartMessage) -> Result<WritingFuture, String> {
        let id = msg.to_str();
        match self.wait_for_channel(id).await {
            Some(_) => self.try_send_message(msg).await,
            None => Err(format!("Failed to wait for channel {id}")),
        }
    }

    pub async fn get_waiter(&self, channel: ChannelId) -> Option<Receiver<()>> {
        match self.senders.lock().await.channel_state(&channel) {
            ChannelState::Uninitialised => {
                Some(self.senders.lock().await.get_creation_waiter(&channel))
            }
            _ => None,
        }
    }

    pub async fn wait_for_channel(&self, channel: ChannelId) -> Option<()> {
        let receiver = self.get_waiter(channel).await;
        if let Some(mut receiver) = receiver {
            receiver.recv().await
        } else {
            Some(())
        }
    }

    pub async fn try_send_message(&self, message: HeartMessage) -> Result<WritingFuture, String> {
        let id = message.to_str();
        match self.senders.lock().await.clone_send(message, &id) {
            Ok(w) => Ok(w),
            _ => Err("Failed to send".to_string()),
        }
    }

    pub async fn send_message(&self, message: HeartMessage) -> WritingFuture {
        let id = message.to_str();
        self.senders.lock().await.clone_send(message, &id).unwrap()
    }

    async fn new_summ(self, bytes: &[u8]) {
        let summ: SummaryMessage = dec!(bytes);
        self.send_message(HeartMessage::EmitSumm(Summaries::from(summ)))
            .await;
    }

    async fn clean(&self, bytes: &[u8]) {
        explicit_log!(self, "Cleaning the pool");

        let started = dec!(bytes);

        match self.pool.clean(Some(started), Some(TIMEOUT)).await {
            Some(mut receiver) => {
                explicit_log!(self, "Waiting for clean to complete..");
                match receiver.recv().await {
                    Some(Err(e)) => {
                        explicit_log!(self, "Cleaning phase over because of timeout: {e:?}");
                    }
                    _ => {
                        explicit_log!(self, "Clean phase over !");
                    }
                }
            }
            None => {
                explicit_log!(self, "Clean is already done");
            }
        }

        self.send_message(HeartMessage::GiveSumm(started))
            .await
            .wait(None)
            .await
            .unwrap();

        let cloned_self = self.clone();
        let (stop_wait_sender, mut stop_wait_receiver) = tokio::sync::oneshot::channel::<()>();

        let force_clean_timer = tokio::spawn(async move {
            explicit_log!(cloned_self, "Sleeping in case of invalid summary...");
            let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(10));
            pin!(sleep);

            tk_select!(
                _ = &mut sleep => {
                        explicit_log!(cloned_self, "Time out for summary waiting");
                        cloned_self
                            .send_message(HeartMessage::ForceClearSumm)
                            .await
                            .wait(None)
                            .await
                            .unwrap();
                        explicit_log!(cloned_self, "Force clear message sent successfully");
                }
                _ = &mut stop_wait_receiver => {
                    explicit_log!(cloned_self, "Waiting for force stop has been canceled");
                }
            )
        });

        explicit_log!(self, "Waiting for summary to finalize");
        let mut receiver = self.subscribe(HeartMessage::NetworkClearedConst).await;
        receiver.recv().await.unwrap();

        let _ = stop_wait_sender.send(());

        explicit_log!(self, "OK for summaries, shutting down");

        self.senders
            .lock()
            .await
            .shutdown_clone(&HeartMessage::NetworkClearedConst)
            .unwrap();

        explicit_log!(
            self,
            "OK for shutdown, sending clean notification to interface"
        );

        let mut msg = vec![InterfaceCode::PoolCleaned.into()];
        msg.append(&mut enc!(self.index()));

        force_clean_timer.await.unwrap();
        self.contact_interface(msg, 0);
    }

    async fn setup(&self, bytes: &[u8]) {
        let (network, base): (Vec<IpV4>, Base) = dec!(bytes);
        let index = network
            .iter()
            .position(|addr| *addr == self.my_ip)
            .expect("Im not in the network") as u16;
        *self.log.lock().await = File::create(&format!("../logs/node_{index}")).unwrap();
        let kp = KeyPair::generate(&base, &mut thread_rng());
        let pk = kp.extract_public_key();
        let _ = self.keypair.lock().await.insert(kp);
        {
            let msg = enc!(Heart, NodeCommand::Key, (index, pk));
            let mut node_network = self.network.lock().await;
            for addr in network.into_iter() {
                async_private_message(addr, msg.clone(), 0, ANONYMOUS);
                node_network.add_ip(addr);
            }
        }

        self.wait_for_channel(HeartMessage::SetupOverConst).await;
        let msg = HeartMessage::SetupOver(SetupData { index, base });
        self.send_message(msg).await;
    }

    pub fn key_waiter(mut self) {
        spawn(async move {
            let mut receiver = self.subscribe(HeartMessage::KeyConst).await;
            self.wait_for_setup().await;
            let (mut key_counter, n) = (0, self.network_size().await);
            let mut keys: Vec<Option<PublicKey>> = vec![None; n as usize];
            loop {
                let msg = break_if_over!(receiver);
                match msg {
                    HeartMessage::Key(i, key) => {
                        assert!(keys[i as usize].is_none());
                        keys[i as usize] = Some(key);
                        key_counter += 1;
                        if key_counter == n {
                            let keys: Vec<PublicKey> =
                                keys.into_iter().map(|k| k.unwrap()).collect();
                            *self.public_keys.lock().await = keys;
                            let msg = vec![InterfaceCode::NodeReady.into()];
                            self.contact_interface(msg, 0);
                            break;
                        }
                    }
                    _ => panic!("Unexpected message"),
                }
            }
        });
    }

    async fn wait_for_setup(&mut self) {
        let channel = HeartMessage::SetupOverConst;
        let mut receiver = self.senders.lock().await.subscribe(&channel, 100);
        match receiver.recv().await.unwrap() {
            HeartMessage::SetupOver(d) => {
                self.save_setup(d);
            }
            _ => panic!("Unexpected setup data"),
        }
    }

    async fn new_key(&self, bytes: &[u8]) {
        let (i, key): (u16, PublicKey) = dec!(bytes);
        let msg = HeartMessage::Key(i, key);
        self.wait_and_send(msg).await;
    }

    async fn kill_myself(&self, _bytes: &[u8]) {
        exit(0)
    }

    pub fn my_ip(&self) -> &IpV4 {
        &self.my_ip
    }

    pub async fn network_size(&self) -> u16 {
        self.network.lock().await.full_len() as u16
    }

    pub fn uindex(&self) -> usize {
        self.index.unwrap() as usize
    }

    pub fn index(&self) -> u16 {
        self.index.unwrap()
    }

    fn contact_interface(&self, msg: Vec<u8>, id: OpId) {
        async_private_message(self.interface_ip, msg, id, ANONYMOUS);
    }

    async fn save_share_set(&self, set: CryptoSet) {
        self.shares_map.lock().await.insert(set.identity(), set);
    }

    async fn get_share_set(&self, ident: CryptoSetIdentity) -> CryptoSet {
        match self.shares_map.lock().await.get(&ident) {
            Some(set) => set.clone(),
            None => panic!(
                "Node {} failed to get the set for the size {ident:?}",
                self.index()
            ),
        }
    }

    fn save_setup(&mut self, d: SetupData) {
        self.index = Some(d.index);
        self.base = Some(d.base);
    }
}
