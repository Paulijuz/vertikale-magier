use super::{
    advertiser::Advertiser,
    socket::{Client, Host, SendableType},
};
use crossbeam_channel::{never, select};
use log::{debug};
use std::{
    net::SocketAddrV4,
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

enum Role<T: SendableType> {
    Master(Host<T>),
    Slave(Client<T>),
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
    let host: Host<String> = Host::new_tcp_host(None);
    let port = host.port();

    let advertiser = Advertiser::init(port);
    advertiser.start_advertising();
    let mut role = Role::Master(host);

    loop {
        let from_slaves_channel = match &role {
            Role::Master(host) => host.receive_channel(),
            _ => &never(),
        };

        let from_master_channel = match &role {
            Role::Slave(client) => client.receiver(),
            _ => &never(),
        };

        select! {
            recv(advertiser.receive_channel()) -> advertisment => {
                let (address, port) = advertisment.unwrap();

                let master_address = SocketAddrV4::new(*address.ip(), port);

                match &role {
                    Role::Master(_) => {
                        debug!("\nFound another master node: {master_address}");
                        advertiser.stop_advertising();

                        debug!("Waiting to connect...");
                        sleep(Duration::from_millis(rand::random_range(0..=100)));

                        if let Ok(client) = Client::new_tcp_client(address.ip().octets(), port) {
                            debug!("Successfully connected!");
                            role = Role::Slave(client);
                            debug!("Now slave.");
                            continue;
                        }

                        debug!("Could not connect to master.");
                        advertiser.start_advertising();
                    },
                    _ => {},
                }
            },
            recv(from_slaves_channel) -> message => {
                debug!("\nData from slave recieved!");

                let (address, data) = message.unwrap();

                debug!("Received data from slave ({address}): {data}");
            },
            recv(from_master_channel) -> message => {
                debug!("\nData from master recieved.");

                let Ok((_, data)) = message else {
                    debug!("Master dead!");

                    let host = Host::new_tcp_host(None);
                    let port = host.port();

                    advertiser.set_advertisment(port);
                    advertiser.start_advertising();

                    role = Role::Master(host);

                    debug!("Now master.");
                    continue;
                };

                debug!("Received data from master: {data}");
            }
        }
    }
}
