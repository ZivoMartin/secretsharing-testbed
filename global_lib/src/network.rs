use crate::{connect, ip_addr::IpV4, private_message, wrap, OpId, Wrapped};
use crate::{KindEvaluation, NodeId};
use rand::{seq::SliceRandom, thread_rng};
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, net::TcpStream};

#[derive(Default)]
pub struct Network {
    network: Vec<Wrapped<Option<TcpStream>>>,
    ips: Vec<IpV4>,
    mode: KindEvaluation,
}

impl Network {
    pub fn new() -> Self {
        Self {
            mode: KindEvaluation::Latency,
            network: Vec::new(),
            ips: Vec::new(),
        }
    }

    pub fn shuffle_ips(&mut self) {
        self.ips.shuffle(&mut thread_rng());
    }

    pub fn switch_on(&mut self, mode: KindEvaluation) {
        self.mode = mode
    }

    pub fn switch_on_debit(&mut self) {
        self.mode = KindEvaluation::Debit;
    }

    pub fn switch_on_latency(&mut self) {
        self.network.clear();
        self.mode = KindEvaluation::Latency
    }

    pub fn ips(&self) -> &Vec<IpV4> {
        &self.ips
    }

    pub fn give_message(&self, index: usize, msg: Vec<u8>, id: OpId, my_id: NodeId) {
        let ip = self.ips[index];
        match self.mode {
            KindEvaluation::Debit => {
                let stream = self.network[index].clone();
                tokio::spawn(async move {
                    let mut stream = stream.lock().await;
                    if stream.is_some() {
                        private_message(stream.as_mut().unwrap(), &msg, id, my_id).await
                    } else {
                        let mut s = connect(&ip).await;
                        private_message(&mut s, &msg, id, my_id).await;
                        let _ = stream.insert(s);
                    }
                });
            }
            KindEvaluation::Latency => {
                tokio::spawn(async move {
                    let mut stream = connect(&ip).await;
                    private_message(&mut stream, &msg, id, my_id).await;
                    stream.shutdown().await.unwrap();
                });
            }
        }
    }

    pub fn message(&self, index: usize, msg: Arc<Vec<u8>>, id: OpId, my_id: NodeId) {
        let ip = self.ips[index];
        match self.mode {
            KindEvaluation::Debit => {
                let stream = self.network[index].clone();
                // println!("Sending msg to {index} as {my_id}");
                tokio::spawn(async move {
                    let mut stream = stream.lock().await;
                    if stream.is_some() {
                        private_message(stream.as_mut().unwrap(), &msg, id, my_id).await
                    } else {
                        let mut s = connect(&ip).await;
                        private_message(&mut s, &msg, id, my_id).await;
                        let _ = stream.insert(s);
                    }
                    // println!("Succed to send msg to {index} as {my_id}")
                });
            }
            KindEvaluation::Latency => {
                tokio::spawn(async move {
                    let mut stream = connect(&ip).await;
                    private_message(&mut stream, &msg, id, my_id).await;
                    stream.shutdown().await.unwrap();
                });
            }
        }
    }

    pub fn add_ip(&mut self, ip: IpV4) {
        self.ips.push(ip)
    }

    pub async fn adjust(&mut self, n: usize) {
        if self.mode == KindEvaluation::Debit && self.network.len() != n {
            self.network = Vec::new();
            for _ in 0..n {
                self.network.push(wrap!(None))
            }
        }
    }

    pub async fn full_connect(&mut self) {
        self.adjust(self.full_len()).await
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.network.len()
    }

    pub fn full_len(&self) -> usize {
        self.ips.len()
    }

    pub fn broadcast(
        &mut self,
        msg: Vec<u8>,
        id: OpId,
        to_contact: Option<Vec<usize>>,
        my_id: NodeId,
    ) {
        let msg = Arc::new(msg);
        match self.mode {
            KindEvaluation::Debit => {
                for i in to_contact.unwrap_or((0..self.len()).collect()) {
                    self.message(i, msg.clone(), id, my_id)
                }
            }
            KindEvaluation::Latency => {
                let ips = self.ips.clone();
                let len = self.full_len();
                tokio::spawn(async move {
                    for i in to_contact.unwrap_or((0..len).collect()) {
                        let mut stream = connect(&ips[i]).await;
                        private_message(&mut stream, &msg, id, my_id).await
                    }
                });
            }
        }
    }

    pub fn extract_subnetwork(&self, n: usize) -> Self {
        Self {
            mode: self.mode,
            network: if self.mode == KindEvaluation::Debit {
                self.network[0..n].to_vec()
            } else {
                Vec::new()
            },
            ips: self.ips[0..n].to_vec(),
        }
    }
}
