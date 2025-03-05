use crate::network::host::ALL_CLIENTS;

use super::{advertiser::Advertiser, client::SendableType, Client, Host};
use crossbeam_channel::{never, select, unbounded, Receiver, Sender};
use log::{debug, info, warn};
use std::{
    net::SocketAddrV4,
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

// Use 52 for group 52 <3
const ADVERTISMENT_IP: [u8; 4] = [239, 0, 0, 52];
const ADVERTISMENT_PORT: u16 = 52000;

enum Role<T: SendableType> {
    Master(Host<T>),
    Slave(Client<T>),
}

pub struct Node<T: SendableType> {
    from_master_channel: Receiver<T>,
    from_slave_channel: Receiver<T>,
    to_master_channel: Sender<T>,
    to_slaves_channel: Sender<T>,
    thread: Option<JoinHandle<()>>,
}

impl<T: SendableType> Node<T> {
    pub fn new() -> Self {
        let (from_master_channel_tx, from_master_channel_rx) = unbounded::<T>();
        let (from_slave_channel_tx, from_slave_channel_rx) = unbounded::<T>();
        let (to_master_channel_tx, to_master_channel_rx) = unbounded::<T>();
        let (to_slave_channel_tx, to_slave_channel_rx) = unbounded::<T>();

        let thread_handle = spawn(move || {
            run_node(
                from_master_channel_tx,
                from_slave_channel_tx,
                to_master_channel_rx,
                to_slave_channel_rx,
            )
        });

        Self {
            from_master_channel: from_master_channel_rx,
            from_slave_channel: from_slave_channel_rx,
            to_slaves_channel: to_slave_channel_tx,
            to_master_channel: to_master_channel_tx,
            thread: Some(thread_handle),
        }
    }

    pub fn to_master_channel(&self) -> &Sender<T> {
        &self.to_master_channel
    }

    pub fn to_slaves_channel(&self) -> &Sender<T> {
        &self.to_slaves_channel
    }

    pub fn from_master_channel(&self) -> &Receiver<T> {
        &self.from_master_channel
    }

    pub fn from_slave_channel(&self) -> &Receiver<T> {
        &self.from_slave_channel
    }
}

impl<T: SendableType> Drop for Node<T> {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().unwrap();
    }
}

fn run_node<T: SendableType>(
    from_master_channel: Sender<T>,
    from_slave_channel: Sender<T>,
    to_master_channel: Receiver<T>,
    to_slave_channel: Receiver<T>,
) {
    let host: Host<T> = Host::new_tcp_host(0).unwrap();
    let port = host.port();

    let advertiser = Advertiser::new(port, ADVERTISMENT_IP, ADVERTISMENT_PORT).unwrap();
    advertiser.start_advertising();

    let mut role = Role::Master(host);

    info!("New node started as master.");

    loop {
        // If the node is a slave it doesn't have a host so we have to set its
        // host receive channel to "never" and vice versa for when the node is a master.
        let host_receive_channel = match &role {
            Role::Master(host) => host.receive_channel(),
            _ => &never(),
        };
        let client_receive_channel = match &role {
            Role::Slave(client) => client.receive_channel(),
            _ => &never(),
        };

        select! {
            recv(advertiser.receive_channel()) -> advertisment => {
                let (address, port) = advertisment.unwrap();

                let master_address = SocketAddrV4::new(*address.ip(), port);

                match &role {
                    Role::Master(_) => {
                        info!("\nFound another master node: {master_address}");
                        advertiser.stop_advertising();

                        debug!("Waiting to connect...");
                        // Wait a random amount of time for arbitration.
                        // The master with the shorter wait time wins!
                        sleep(Duration::from_millis(rand::random_range(0..=100)));
                        // TODO: Find a better way to arbitrate masters.

                        if let Ok(client) = Client::new_tcp_client(address.ip().octets(), port) {
                            role = Role::Slave(client);
                            info!("Successfully connected to master! Now slave.");
                            continue;
                        }

                        info!("Could not connect to master.");
                        advertiser.start_advertising();
                    },
                    _ => {},
                }
            },
            recv(host_receive_channel) -> message => {
                debug!("\nData from slave recieved!");

                if matches!(role, Role::Slave(_)) {
                    panic!("A slave should not be able to receive a message from another slave.")
                }

                let (_, data) = message.unwrap();

                from_slave_channel.send(data).unwrap();
            },
            recv(client_receive_channel) -> message => {
                debug!("\nData from master recieved.");

                if matches!(role, Role::Master(_)) {
                    panic!("A master should not be able to receive a message from another master.")
                }

                let Ok((_, data)) = message else {
                    info!("Master is dead!");

                    let host = Host::new_tcp_host(0).unwrap();
                    let port = host.port();

                    advertiser.set_advertisment(port);
                    advertiser.start_advertising();

                    role = Role::Master(host);

                    info!("Now master.");
                    continue;
                };

                from_master_channel.send(data).unwrap();
            },
            recv(to_slave_channel) -> message => {
                let message = message.unwrap();

                match &role {
                    Role::Master(host) => {
                        from_master_channel.send(message.clone()).unwrap();
                        host.send_channel().send((ALL_CLIENTS, message)).unwrap();
                    },
                    Role::Slave(_) => warn!("Tried sending to slaves while being a slave node."),
                };
            },
            recv(to_master_channel) -> message => {
                let message = message.unwrap();

                // Send back to our selves if we are the master node.
                match &role {
                    Role::Master(_) => from_slave_channel.send(message).unwrap(),
                    Role::Slave(client) => client.send_channel().send(message).unwrap(),
                };
            },
        }
    }
}
