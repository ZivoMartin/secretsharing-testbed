pub mod config_treatment;
pub mod ip_addr;
pub mod macros;
pub mod messages;
pub mod network;
pub mod process;
pub mod process_pool;
pub mod settings;
pub mod task_pool;

use crate::messages::NameSpace;
use ip_addr::IpV4;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufWriter, Write},
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

pub type OpId = u64;

pub type NodeId = u16;
// Default index
pub const ANONYMOUS: u16 = 999;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum Evaluation {
    Debit(Step),
    Latency(Step),
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum KindEvaluation {
    Debit,
    #[default]
    Latency,
}

impl From<KindEvaluation> for String {
    fn from(kind: KindEvaluation) -> String {
        String::from(match kind {
            KindEvaluation::Debit => "debit",
            KindEvaluation::Latency => "latency",
        })
    }
}

pub type Wrapped<T> = Arc<Mutex<T>>;

as_number!(
    u8,
    enum ByzStatus {
        Byz,
        Honnest,
    }
);

impl Default for Evaluation {
    fn default() -> Self {
        Evaluation::Latency(Step::Sharing)
    }
}

impl KindEvaluation {
    fn to_eval(self, step: Step) -> Evaluation {
        match self {
            KindEvaluation::Debit => Evaluation::Debit(step),
            KindEvaluation::Latency => Evaluation::Latency(step),
        }
    }
}

impl Evaluation {
    pub fn get_kind(&self) -> KindEvaluation {
        match self {
            Evaluation::Debit(_) => KindEvaluation::Debit,
            Evaluation::Latency(_) => KindEvaluation::Latency,
        }
    }

    pub fn is_latency(&self) -> bool {
        matches!(self, Evaluation::Latency(_))
    }

    pub fn is_debit(&self) -> bool {
        matches!(self, Evaluation::Debit(_))
    }

    pub fn is_reconstruct(&self) -> bool {
        let step = match self {
            Evaluation::Debit(s) => s,
            Evaluation::Latency(s) => s,
        };
        *step == Step::Reconstruct
    }

    pub fn change_step(&mut self, step: Step) {
        *self = match self {
            Evaluation::Debit(_) => Evaluation::Debit(step),
            Evaluation::Latency(_) => Evaluation::Latency(step),
        };
    }

    pub fn get_step(&self) -> Step {
        match self {
            Evaluation::Debit(s) => *s,
            Evaluation::Latency(s) => *s,
        }
    }
}

#[derive(PartialEq, Hash, Eq, Clone, Serialize, Deserialize, Copy, Debug, Default)]
pub enum Step {
    Sharing,
    #[default]
    Reconstruct,
}

impl Step {
    fn all() -> Vec<Self> {
        vec![Self::Sharing, Self::Reconstruct]
    }
}

impl From<Step> for String {
    fn from(step: Step) -> String {
        String::from(match step {
            Step::Sharing => "sharing",
            Step::Reconstruct => "reconstruct",
        })
    }
}
impl From<&str> for Step {
    fn from(s: &str) -> Step {
        match s {
            "sharing" => Step::Sharing,
            "reconstruct" => Step::Reconstruct,
            _ => panic!("Unvalid step string"),
        }
    }
}

pub fn async_private_message(ip: IpV4, message: Vec<u8>, id: u64, my_id: u16) {
    tokio::spawn(
        async move { private_message(&mut connect(&ip).await, &message, id, my_id).await },
    );
}

pub async fn connect(addr: &IpV4) -> TcpStream {
    match TcpStream::connect(&addr.to_string()).await {
        Ok(stream) => stream,
        Err(e) => panic!("Failed to connect to {}: {}", addr, e),
    }
}

#[derive(Serialize, Deserialize)]
pub struct MetaInfo {
    length: u32,
    op_id: u64,
    sender_id: NodeId,
}
const META_INFO_LENGTH: usize = 14;

pub async fn generate_random_port(ip: &str) -> (u16, TcpListener) {
    let listener = TcpListener::bind(&format!("{ip}:0")).await.unwrap();
    let SocketAddr::V4(addr) = listener.local_addr().unwrap() else {
        panic!()
    };
    (addr.port(), listener)
}

pub fn write_in_file(path: &str, content: &str) {
    let file = File::create(path).unwrap_or_else(|_| panic!("Failed to create file {path}"));
    let mut writer = BufWriter::new(file);
    writer
        .write_all(content.as_bytes())
        .expect("Failed to write");
    writer.flush().expect("Failed to flush");
}

pub async fn private_message_multiple(
    stream: &mut TcpStream,
    messages: Vec<&[u8]>,
    op_id: u64,
    my_id: u16,
) {
    let metinfo = enc!(MetaInfo {
        length: messages.iter().map(|m| m.len() as u32).sum(),
        op_id,
        sender_id: my_id,
    });
    stream.write_all(&metinfo).await.unwrap();
    for message in messages {
        stream.write_all(&message).await.unwrap();
    }
    stream.flush().await.expect("Failed to flush");
}

pub async fn private_message(stream: &mut TcpStream, message: &[u8], op_id: u64, my_id: u16) {
    let metinfo = enc!(MetaInfo {
        length: message.len() as u32,
        op_id,
        sender_id: my_id,
    });
    stream.write_all(&metinfo).await.unwrap();
    stream.write_all(&message).await.unwrap();
    stream.flush().await.expect("Failed to flush");
}

pub async fn get_next_message(socket: &mut TcpStream) -> Option<(Vec<u8>, NodeId, OpId)> {
    let mut metainfo = [0; META_INFO_LENGTH];
    if socket.read_exact(&mut metainfo).await.is_err() {
        return None;
    }
    let metainfo: MetaInfo = dec!(metainfo);

    let mut message_buf = vec![0; metainfo.length as usize];
    if let Err(e) = socket.read_exact(&mut message_buf).await {
        println!("WARNING: Failed to receiv the entire message: {e}");
        return None;
    }
    Some((message_buf, metainfo.sender_id, metainfo.op_id))
}

pub fn init_message(namespace: NameSpace, command: impl Into<u8>) -> Vec<u8> {
    vec![namespace.into(), command.into()]
}
