use super::{advertiser::Advertiser, socket::Host};
use log::warn;
use petname::Generator;
use std::{
    net::SocketAddr,
    str::FromStr,
    thread::{spawn, JoinHandle},
};

enum Role {
    Master,
    Slave,
}

pub struct Node {
    role: Role,
    thread: JoinHandle<()>,
}

impl Node {
    pub fn init() -> Self {
        let thread = spawn(move || run_node());

        Node {
            role: Role::Master,
            thread,
        }
    }

    pub fn status_channel(&self) {}

    pub fn send_channel(&self) {}

    pub fn receive_channel(&self) {}
}

fn run_node() {
    let host = Host::new_tcp_host(None);
    let port = host.port();

    let advertisment = format!("MASTER: {port}");
    let advertiser = Advertiser::init(&advertisment);
    advertiser.start_advertising();

    loop {
        let (address, data) = advertiser.receive_channel().recv().unwrap();

        let Some(port) = data.strip_prefix("MASTER: ") else {
            println!("Received garbage: {data}");
            continue;
        };

        let Ok(port) = port.parse::<u16>() else {
            warn!("Received invalid port: {port}");
            continue;
        };

        let master_address = SocketAddr::new(address.as_socket().unwrap().ip(), port);

        println!("Found a master node: {master_address}");

        // advertiser.stop_advertising();
    }
}