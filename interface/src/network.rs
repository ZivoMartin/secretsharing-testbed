use crate::base_generator::generate_random_base;
use global_lib::{
    async_private_message, enc,
    ip_addr::IpV4,
    messages::{ManagerCode, NodeCommand},
    network::Network as PrimitiveNetwork,
    wrap, wrapper_impl, OpId, Wrapped, ANONYMOUS,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};
enum NetworkMessage {
    Ip(IpV4),
    Ready,
}

struct WrappedNetwork {
    network: PrimitiveNetwork,
    sender: Sender<NetworkMessage>,
    ready_counter: usize,
}

impl WrappedNetwork {
    fn new() -> Wrapped<Self> {
        let (sender, receiver) = channel(100);
        let net = wrap!(WrappedNetwork {
            network: PrimitiveNetwork::new(),
            ready_counter: 0,
            sender
        });
        Self::listen_for_new_node(net.clone(), receiver);
        net
    }

    fn listen_for_new_node(network: Wrapped<Self>, mut receiver: Receiver<NetworkMessage>) {
        tokio::spawn(async move {
            loop {
                match receiver.recv().await.unwrap() {
                    NetworkMessage::Ip(ip) => {
                        let mut network = network.lock().await;
                        network.network.add_ip(ip)
                    }
                    NetworkMessage::Ready => network.lock().await.new_node_ready(),
                }
            }
        });
    }

    fn new_node_ready(&mut self) {
        self.ready_counter += 1;
    }

    async fn add_node(&self, ip: IpV4) {
        self.sender
            .send(NetworkMessage::Ip(ip))
            .await
            .expect("Failed to send the ip of the new node")
    }

    pub async fn new_ready(&self) {
        self.sender
            .send(NetworkMessage::Ready)
            .await
            .expect("Failed to send the ip of the new node")
    }

    async fn generate_nodes(&mut self, managers_ip: &Vec<IpV4>, n: usize) {
        let nb_manager = managers_ip.len();
        let mut manager_index = 0;
        let mut node_distribution = vec![0; nb_manager];
        for i in 0..n {
            node_distribution[i % nb_manager] += 1;
        }
        for node_number in node_distribution {
            if node_number == 0 {
                break;
            }
            let mut msg = vec![ManagerCode::Gen.into()];
            enc!(node_number, msg);
            async_private_message(managers_ip[manager_index], msg, 0, ANONYMOUS);
            manager_index = (1 + manager_index) % nb_manager
        }
    }

    async fn wait_for_nodes_connection(network: &Wrapped<Self>, n: usize) {
        while network.lock().await.network.full_len() < n {}
        network.lock().await.network.shuffle_ips();
    }

    async fn broadcast_setup_message(&mut self) {
        let ips = self.network.ips();
        let msg = enc!(Heart, NodeCommand::Setup, (ips, generate_random_base()));
        self.broadcast(msg, 0, None).await;
    }

    async fn wait_for_ready(network: &Wrapped<Self>, n: usize) {
        while network.lock().await.ready_counter != n {}
    }

    async fn init_network(network: &Wrapped<Self>, n: u16, managers_ip: &Vec<IpV4>) {
        let n = n as usize;
        println!("Init the network with {n} nodes");
        network.lock().await.generate_nodes(managers_ip, n).await;
        Self::wait_for_nodes_connection(network, n).await;
        println!("All the nodes are connected");
        network.lock().await.broadcast_setup_message().await;
        Self::wait_for_ready(network, n).await;
        println!("Network is ready");
    }

    async fn broadcast(&mut self, msg: Vec<u8>, id: OpId, n: Option<usize>) {
        let n = if let Some(n) = n {
            self.network.adjust(n).await;
            Some((0..n).collect())
        } else {
            self.network.full_connect().await;
            None
        };
        self.network.broadcast(msg, id, n, ANONYMOUS);
        self.network.adjust(0).await;
    }

    pub async fn switch_on_latency(&mut self) {
        self.network.switch_on_latency()
    }
}

wrapper_impl!(Network, WrappedNetwork, network,
       ;self_meth,
              new_ready
              add_node, ip : IpV4
              broadcast, msg : Vec<u8>, id : OpId, n : Option<usize>
              switch_on_latency
       ;by_name_space,
    init_network, n : u16, managers_ip : &Vec<IpV4>
);
