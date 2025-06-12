use global_lib::{
    async_private_message, enc, generate_random_port, get_next_message,
    ip_addr::{Ip, IpV4},
    messages::{InterfaceCode, ManagerCode},
    settings::MANAGER_PORT,
    ANONYMOUS,
};
use nodes::system::nodes_heart::NodesHeart;
use std::{env, panic, str::FromStr};
use tokio::net::TcpListener;

fn connect_to_manager(interface_ip: IpV4, manager_ip: Ip, port: u16) {
    println!("{interface_ip} {manager_ip:?}");
    let mut buf = vec![ManagerCode::Connect.into()];
    enc!(std::process::id(), buf);
    let manager_ip = IpV4::new(manager_ip, MANAGER_PORT);
    async_private_message(manager_ip, buf.clone(), 0, ANONYMOUS);

    buf = vec![InterfaceCode::Connect.into()];
    enc!(port, buf);
    async_private_message(interface_ip, buf, 0, ANONYMOUS);
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let interface_ip = IpV4::from_str(&env::args().nth(1).unwrap()).unwrap();
    let my_ip = IpV4::ip_from_str(&env::args().nth(2).unwrap())?;
    let (port, listener) = generate_random_port(&IpV4::ip_to_string(&my_ip)).await;
    let heart = NodesHeart::new(interface_ip, IpV4::new(my_ip, port)).await;
    connect_to_manager(interface_ip, my_ip, port);
    listen_with(listener, heart).await;
    Ok(())
}

async fn listen_with(listener: TcpListener, heart: NodesHeart) {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let heart = heart.clone();
        tokio::spawn(async move {
            loop {
                if let Some((message_buf, sender, id)) = get_next_message(&mut socket).await {
                    heart.new_message(message_buf, sender, id).await;
                } else {
                    break;
                }
            }
        });
    }
}
