use crossbeam_channel::{select, tick, unbounded, Receiver, Sender};
use log::{debug, error, warn};
use serde::{de, Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};

use std::{
    io::{ErrorKind, Read, Result},
    net::{Ipv4Addr, Shutdown, SocketAddrV4},
    thread::{spawn, JoinHandle},
    time::{Duration, Instant},
};

const BUFFER_SIZE: usize = 16384; // 16 kB
const DELIMITER: &str = " | DELIMITER | ";
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(50);
const CONNECTION_TIMEOUT: Duration = Duration::from_millis(500);

// Define an empty trait to use as an alias for all of the traits below
pub trait Transmit: Serialize + de::DeserializeOwned + Clone + Send + 'static {}
impl<T: Serialize + de::DeserializeOwned + Clone + Send + 'static> Transmit for T {}

pub struct Client<T: Transmit> {
    socket: Socket,
    send_channel: Option<Sender<T>>,
    receive_channel: Receiver<(SocketAddrV4, T)>,
    send_thread: Option<JoinHandle<()>>,
    receive_thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message<T> {
    Data(T),
    Heartbeat,
}

fn receive<T: Transmit>(mut socket: Socket, receive_channel_tx: Sender<(SocketAddrV4, T)>) {
    let mut last_received: Option<Instant> = None;
    let mut parse_buffer: String = String::new();

    loop {
        let mut receive_buffer = [0; BUFFER_SIZE];

        let (Ok(address), Ok(count)) = (socket.peek_sender(), socket.read(&mut receive_buffer))
        else {
            break;
        };

        if count == 0 {
            break;
        }

        let address = address
            .as_socket_ipv4()
            .unwrap_or(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));

        parse_buffer.push_str(&String::from_utf8_lossy(&receive_buffer[..count]));

        while let Some(i) = parse_buffer.find(DELIMITER) {
            let drain: String = parse_buffer.drain(..i).collect();
            parse_buffer.drain(..DELIMITER.len());

            match serde_json::from_str(&drain) {
                //Splitter mellom at det er data eller heartbeat
                Ok(Message::Data(data)) => receive_channel_tx.send((address, data)).unwrap(),
                Ok(Message::Heartbeat) => last_received = Some(Instant::now()),
                Err(error) => {
                    let data = String::from_utf8_lossy(&receive_buffer[..count]);
                    warn!(
                        "Could not deserialize received data!\nData: {}\nError: {:?}",
                        data, error
                    );
                }
            }
        }

        //Midlertidig løsning for å sjekke om heisen er i live
        if let Some(last) = last_received {
            if last.elapsed() > CONNECTION_TIMEOUT {
                error!(
                    "No heartbeats received for {} ms, it's dead",
                    last.elapsed().as_millis()
                );
            }
        }
    }
}

fn send<T: Transmit>(socket: Socket, send_channel_rx: Receiver<T>, send_address: SocketAddrV4) {
    let ticker = tick(HEARTBEAT_INTERVAL);

    loop {
        let message = select! {
            // Send heartbeat, transform to JSON.
            recv(ticker) -> _ => Message::Heartbeat,
            recv(send_channel_rx) -> data => {
                let Ok(data) = data else {
                    break;
                };
                Message::Data(data)
            },
        };

        let Ok(mut buffer) = serde_json::to_vec(&message) else {
            panic!("Could not serialize message!");
        };

        buffer.extend(DELIMITER.as_bytes());

        if socket.send_to(&buffer, &send_address.into()).is_err() {
            warn!("Could not send on socket. Dropping message.");
        }
    }
}

impl<T: Transmit> Client<T> {
    pub fn new(socket: Socket, send_address: SocketAddrV4) -> Result<Self> {
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<T>();

        let receive_socket = socket.try_clone()?;
        let receive_thread_handle = spawn(move || receive(receive_socket, receive_channel_tx));
        let send_socket = socket.try_clone()?;
        let send_thread_handle = spawn(move || send(send_socket, send_channel_rx, send_address));

        Ok(Client {
            socket,
            send_channel: Some(send_channel_tx),
            receive_channel: receive_channel_rx,
            send_thread: Some(send_thread_handle),
            receive_thread: Some(receive_thread_handle),
        })
    }
    pub fn new_udp_multicast_client(multicast_ip: [u8; 4], port: u16) -> Result<Self> {
        let multicast_ip = Ipv4Addr::from(multicast_ip);
        let address = SocketAddrV4::new(multicast_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        socket.bind(&address.into())?;
        socket.join_multicast_v4(&multicast_ip, &Ipv4Addr::UNSPECIFIED)?;

        Client::new(socket, address)
    }
    pub fn new_tcp_client(host_ip: [u8; 4], port: u16) -> Result<Self> {
        let host_ip = Ipv4Addr::from(host_ip);
        let address = SocketAddrV4::new(host_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        socket.connect(&address.into())?;

        Client::new(socket, address)
    }
    pub fn send_channel(&self) -> &Sender<T> {
        self.send_channel
            .as_ref()
            .expect("Send channel should exist as long as client exists.")
    }
    pub fn receive_channel(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receive_channel
    }
}

impl<T: Transmit> Drop for Client<T> {
    fn drop(&mut self) {
        debug!("Shutting down client...");
        self.socket
            .shutdown(Shutdown::Both)
            .unwrap_or_else(|error| {
                if error.kind() != ErrorKind::NotConnected {
                    panic!("Could not shutdown socket: {error:?}");
                }
            });
        drop(self.send_channel.take().unwrap());

        self.send_thread.take().unwrap().join().unwrap();
        self.receive_thread.take().unwrap().join().unwrap();
        debug!("Client shut down.")
    }
}
