use super::{
    advertiser::Advertiser,
    client::Client,
    client::Transmit,
    host::{Host, ALL_CLIENTS},
};
use crossbeam_channel::{never, select, unbounded, Receiver, Sender};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    net::SocketAddrV4,
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

// Use 52 for group 52 <3
const NODE_ADVERTISMENT_IP: [u8; 4] = [239, 0, 0, 52];
const NODE_ADVERTISMENT_PORT: u16 = 52000;

enum State<T: Transmit> {
    Host(Host<T>),
    Client(Client<T>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Slave,
    Master(HashSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct NodeAdvertisment {
    name: String,
    port: Option<u16>,
}

pub struct Node<T: Transmit> {
    connection_upate_channel: Receiver<Role>,
    from_master_channel: Receiver<T>,
    from_slave_channel: Receiver<T>,
    to_master_channel: Sender<T>,
    to_slaves_channel: Sender<T>,
    shutdown_channel: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl<T: Transmit> Node<T> {
    pub fn new(name: String) -> Self {
        let (connection_upate_channel_tx, connection_upate_channel_rx) =
            unbounded::<Role>();
        let (from_master_channel_tx, from_master_channel_rx) = unbounded::<T>();
        let (from_slave_channel_tx, from_slave_channel_rx) = unbounded::<T>();
        let (to_master_channel_tx, to_master_channel_rx) = unbounded::<T>();
        let (to_slaves_channel_tx, to_slaves_channel_rx) = unbounded::<T>();
        let (shutdown_channel_tx, shutdown_channel_rx) = unbounded::<()>();

        let thread_handle = spawn(move || {
            run_node(
                name,
                connection_upate_channel_tx,
                from_master_channel_tx,
                from_slave_channel_tx,
                to_master_channel_rx,
                to_slaves_channel_rx,
                shutdown_channel_rx,
            )
        });

        Self {
            connection_upate_channel: connection_upate_channel_rx,
            from_master_channel: from_master_channel_rx,
            from_slave_channel: from_slave_channel_rx,
            to_master_channel: to_master_channel_tx,
            to_slaves_channel: to_slaves_channel_tx,
            shutdown_channel: shutdown_channel_tx,
            thread: Some(thread_handle),
        }
    }

    pub fn connection_update_channel(&self) -> &Receiver<Role> {
        &self.connection_upate_channel
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

impl<T: Transmit> Drop for Node<T> {
    fn drop(&mut self) {
        debug!("Shutting down node...");
        self.shutdown_channel.send(()).unwrap();
        self.thread.take().unwrap().join().unwrap();
        debug!("Node shut down.")
    }
}

fn run_node<T: Transmit>(
    name: String,
    connection_update_channel: Sender<Role>,
    from_master_channel: Sender<T>,
    from_slave_channel: Sender<T>,
    to_master_channel: Receiver<T>,
    to_slaves_channel: Receiver<T>,
    shutdown_channel: Receiver<()>,
) {
    let host = Host::<T>::new_tcp_host(0).unwrap();
    let mut port = host.port();

    let advertisment = NodeAdvertisment {
        name: name.clone(),
        port: Some(port),
    };

    let advertiser = Advertiser::new(
        Some(advertisment),
        NODE_ADVERTISMENT_IP,
        NODE_ADVERTISMENT_PORT,
    )
    .unwrap();
    advertiser.start_advertising();

    let mut state = State::Host(host);
    connection_update_channel
        .send(Role::Master(HashSet::from([name.clone()])))
        .unwrap();

    info!("New node started as master on port {port}.");

    loop {
        // If the node is a slave it doesn't have a host so we have to set its
        // host receive channel to "never" and vice versa for when the node is a master.
        let host_receive_channel = match &state {
            State::Host(host) => host.receive_channel(),
            _ => &never(),
        };
        let client_receive_channel = match &state {
            State::Client(client) => client.receive_channel(),
            _ => &never(),
        };

        select! {
            recv(advertiser.receive_channel()) -> advertisments => {
                let advertisments = advertisments.unwrap();

                if matches!(state, State::Client(_)) {
                    continue;
                }

                let mut names: HashSet<String> = advertisments.iter().map(|(_, advertisment)| advertisment.name.clone()).collect();
                names.insert(name.clone());
                connection_update_channel.send(Role::Master(names)).unwrap();

                for (address, advertisment) in advertisments {
                    let Some(advertised_port) = advertisment.port else {
                        continue;
                    };

                    if name > advertisment.name {
                        continue;
                    } 

                    advertiser.set_advertisment(NodeAdvertisment {
                        name: name.clone(),
                        port: None,
                    });

                    let master_address = SocketAddrV4::new(*address.ip(), advertised_port);

                    info!("Found eligible master node \"{}\": {}", advertisment.name, master_address);
                    
                    if let Ok(client) = Client::new_tcp_client(address.ip().octets(), advertised_port) {
                        state = State::Client(client);
                        connection_update_channel.send(Role::Slave).unwrap();
                        info!("Successfully connected to master \"{}\"! Now slave.", advertisment.name);
                        break;
                    }

                    info!("Could not connect to master \"{}\".", advertisment.name);

                    advertiser.set_advertisment(NodeAdvertisment {
                        name: name.clone(),
                        port: Some(port),
                    });
                }
            },
            recv(host_receive_channel) -> message => {
                debug!("Data from slave recieved!");

                if matches!(state, State::Client(_)) {
                    panic!("A slave should not be able to receive a message from another slave.")
                }

                let (_, data) = message.unwrap();

                from_slave_channel.send(data).unwrap();
            },
            recv(client_receive_channel) -> message => {
                debug!("Data from master recieved.");

                if matches!(state, State::Host(_)) {
                    panic!("A master should not be able to receive a message from another master.")
                }

                let Ok((_, data)) = message else {
                    info!("Master is dead!");

                    let host = Host::new_tcp_host(0).unwrap();
                    port = host.port();

                    advertiser.set_advertisment(NodeAdvertisment {
                        name: name.clone(),
                        port: Some(port),
                    });

                    state = State::Host(host);
                    connection_update_channel.send(Role::Master(HashSet::from([name.clone()]))).unwrap();

                    info!("Now master on port {port}.");
                    continue;
                };

                from_master_channel.send(data).unwrap();
            },
            recv(to_slaves_channel) -> message => {
                let message = message.unwrap();

                match &state {
                    State::Host(host) => {
                        from_master_channel.send(message.clone()).unwrap();
                        host.send_channel().send((ALL_CLIENTS, message)).unwrap();
                    },
                    State::Client(_) => warn!("Tried sending to slaves while being a slave node."),
                };
            },
            recv(to_master_channel) -> message => {
                let message = message.unwrap();

                // Send back to our selves if we are the master node.
                match &state {
                    State::Host(_) => from_slave_channel.send(message).unwrap(),
                    State::Client(client) => client.send_channel().send(message).unwrap(),
                };
            },
            recv(shutdown_channel) -> _ => {
                break;
            }
        }
    }
}
