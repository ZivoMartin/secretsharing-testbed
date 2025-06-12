use global_lib::{
    dec, get_next_message,
    ip_addr::{Ip, IpV4},
    messages::ManagerCode,
    select,
    settings::{INTERFACE_PORT, LOCAL, LOCAL_IP, MANAGER_PORT},
    Wrapped,
};
use std::{str::FromStr, sync::Arc};
use sysinfo::System;
use tokio::{net::TcpListener, process::Command, sync::Mutex};
fn cpu_usage_loger() {
    tokio::spawn(async move {
        let mut sys = System::new_all();
        sys.refresh_all();
        // let mut log = std::fs::File::create("../logs/cpu").unwrap();
        loop {
            sys.refresh_cpu_all();
            // for (i, cpu) in sys.cpus().iter().enumerate() {
            //     writeln!(log, "cpu {i}: {}% ", cpu.cpu_usage()).unwrap();
            // }
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        }
    });
}

#[allow(dead_code)]
struct Manager {
    machin_ip: Ip,
    interface_ip: IpV4,
    nodes: Vec<u16>,
    n_to_reach: u16,
}

impl Manager {
    fn new(ip: Ip) -> Manager {
        Manager {
            machin_ip: ip,
            nodes: Vec::new(),
            interface_ip: IpV4::default(),
            n_to_reach: 0,
        }
    }
}

async fn new_command(manag: Wrapped<Manager>, bytes: Vec<u8>, ip: String) {
    select!(
        ManagerCode, bytes, manag,
        Gen => generate ip,
        Connect => add_node,
    );
}

async fn generate(manag: Wrapped<Manager>, bytes: &[u8], interface_ip: String) {
    for node in manag.lock().await.nodes.drain(..) {
        Command::new("kill")
            .arg("-9")
            .arg(node.to_string())
            .status()
            .await
            .expect("Failed to create a new node");
    }

    let n: u16 = dec!(bytes, u16);
    let machin_ip = manag.lock().await.machin_ip;
    for _ in 0..n {
        let ip = interface_ip.clone();
        tokio::spawn(async move {
            Command::new("../target/release/nodes")
                .arg(&ip)
                .arg(IpV4::ip_to_string(&machin_ip))
                .status()
                .await
                .expect("Failed to create a new node");
        });
    }
    manag.lock().await.n_to_reach = n;
    manag.lock().await.interface_ip = IpV4::from_str(&interface_ip).unwrap();
    println!("{} nodes generated.", n);
}

async fn add_node(manag: Wrapped<Manager>, bytes: &[u8]) {
    let mut manag = manag.lock().await;
    let id: u16 = dec!(bytes, u16);
    manag.nodes.push(id);
}

#[tokio::main]
async fn main() -> Result<(), String> {
    cpu_usage_loger();
    let (ip, listener) = if LOCAL {
        (
            IpV4::LOCAL_IP,
            TcpListener::bind(IpV4::new(IpV4::LOCAL_IP, MANAGER_PORT).to_string())
                .await
                .expect("Failed to bind"),
        )
    } else {
        let ip = IpV4::ip_from_str(
            &local_ip_address::local_ip()
                .expect("Failed to catch ip")
                .to_string(),
        )?;
        if let Ok(l) = TcpListener::bind(IpV4::new(ip, MANAGER_PORT).to_string()).await {
            (ip, l)
        } else {
            eprintln!("Failed to connect to {ip:?}");
            std::process::exit(1);
        }
    };
    let manag = Arc::new(Mutex::new(Manager::new(ip)));
    loop {
        let (mut socket, ip) = listener.accept().await.unwrap();
        let manag = manag.clone();
        tokio::spawn(async move {
            loop {
                let (message_buf, _, _) = match get_next_message(&mut socket).await {
                    Some(b) => b,
                    _ => return,
                };
                let ip = format!(
                    "{}:{INTERFACE_PORT}",
                    if LOCAL {
                        LOCAL_IP.to_string()
                    } else {
                        ip.to_string()
                            .split(":")
                            .next()
                            .expect("Invalid interface ip")
                            .to_string()
                    }
                );
                new_command(manag.clone(), message_buf, ip).await;
            }
        });
    }
}
