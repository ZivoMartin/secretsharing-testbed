use crate::private_message;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::{fmt::Display, str::FromStr};
use tokio::net::{TcpListener, TcpStream};
pub async fn broadcast(network: &mut [TcpStream], message: Vec<u8>, id: u64, my_id: u16) {
    let msg = Arc::new(message);
    for node in network.iter_mut() {
        private_message(node, &msg, id, my_id).await;
    }
}

pub type Ip = [u8; 4];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct IpV4 {
    ip: Ip,
    port: u16,
}

impl IpV4 {
    pub const LOCAL_IP: Ip = [127, 0, 0, 1];

    pub fn ip_to_string(ip: &Ip) -> String {
        format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
    }

    pub fn in_local(port: u16) -> Self {
        IpV4::new(Self::LOCAL_IP, port)
    }

    pub fn new(ip: Ip, port: u16) -> Self {
        IpV4 { ip, port }
    }

    pub fn extract_ip_from_str(s: &str) -> Result<Ip, String> {
        let mut ip_iter = s.split(':');
        Self::ip_from_str(ip_iter.next().unwrap())
    }

    pub fn ip_from_str(s: &str) -> Result<Ip, String> {
        let host = s.split('@').last().unwrap_or(s);

        if let Ok(parsed) = host.parse::<IpAddr>() {
            return match parsed {
                IpAddr::V4(ipv4) => Ok(ipv4.octets()),
                IpAddr::V6(_) => Err(format!("IPv6 not supported: {host}")),
            };
        }

        let addr = format!("{host}:0");
        let resolved = addr
            .to_socket_addrs()
            .map_err(|_| format!("DNS resolution failed for: {host}"))?;

        for socket in resolved {
            if let IpAddr::V4(ipv4) = socket.ip() {
                return Ok(ipv4.octets());
            }
        }

        Err(format!("No IPv4 address found for: {host}"))
    }
}

impl FromStr for IpV4 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut ip_iter = s.split(':');
        let ip = Self::ip_from_str(ip_iter.next().unwrap())?;
        let port = ip_iter.next().unwrap().parse::<u16>().expect("Invalid ip");
        Ok(IpV4::new(ip, port))
    }
}

impl Display for IpV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", IpV4::ip_to_string(&self.ip), self.port)
    }
}

pub fn extract_ip(addr: &str) -> String {
    extract_in_ip(addr, 0)
}

pub fn extract_port(addr: &str) -> u16 {
    extract_in_ip(addr, 1)
        .parse::<u16>()
        .expect("Failed to parse port in number")
}

fn extract_in_ip(addr: &str, part: usize) -> String {
    addr.split(':')
        .nth(part)
        .expect("Invalid address ip")
        .to_string()
}

pub async fn generate_random_port(ip: &str) -> (u16, TcpListener) {
    let listener = TcpListener::bind(&format!("{ip}:0")).await.unwrap();
    let SocketAddr::V4(addr) = listener.local_addr().unwrap() else {
        panic!()
    };
    (addr.port(), listener)
}
