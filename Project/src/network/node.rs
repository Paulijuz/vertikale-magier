use super::{
    advertiser::Advertiser,
    socket::{Client, Host},
};
use crossbeam_channel::{never, select};
use log::warn;
use std::{
    net::{SocketAddr, SocketAddrV4},
    thread::{spawn, JoinHandle},
};

enum Role {
    Master(Host),
    Slave(Client),
}

pub struct Node {
    thread: JoinHandle<()>,
}

impl Node {
    pub fn init() -> Self {
        let thread = spawn(move || run_node());

        Node { thread }
    }

    pub fn status_channel(&self) {}

    pub fn send_channel(&self) {}

    pub fn receive_channel(&self) {}
}

fn run_node() {
    let host = Host::new_tcp_host(None);
    let port = host.port();

    let advertisment: String = format!("MASTER: {port}");
    let advertiser = Advertiser::init(&advertisment);
    advertiser.start_advertising();

    let mut role = Role::Master(host);

    loop {
        let from_slaves_channel = match &role {
            Role::Master(host) => host.receiver(),
            _ => &never(),
        };

        let from_master_channel = match &role {
            Role::Slave(client) => client.receiver(),
            _ => &never(),
        };
        
        select! {
            recv(advertiser.receive_channel()) -> advertisment => {
                let (address, data) = advertisment.unwrap();

                // TODO: This should be part of the advertiser/socket modules:
                let Some(port) = data.strip_prefix("MASTER: ") else {
                    println!("Received garbage: {data}");
                    continue;
                };

                let Ok(port) = port.parse::<u16>() else {
                    warn!("Received invalid port: {port}");
                    continue;
                };

                let master_address = SocketAddrV4::new(*address.ip(), port);

                match &role {
                    Role::Master(_) => {
                        println!("Found another master node: {master_address}");

                        advertiser.stop_advertising();

                        role = Role::Slave(
                            Client::new_tcp_client(address.ip().octets(), port)
                        );

                        println!("Successfully connected!");
                    },
                    _ => {},
                }
            },
            recv(from_slaves_channel) -> message => {
                let (address, data) = message.unwrap();
                
                println!("Received data from slave ({address}): {data}");
            },
            recv(from_master_channel) -> message => {
                let Ok((_, data)) = message else {
                    let host = Host::new_tcp_host(None);

                    role = Role::Master(host);

                    println!("Master disconnected!");
                    continue;
                };

                println!("Received data from master: {data}");
            }
        }
    }
}

fn run_node_slave() {}

fn run_node_master() {}
