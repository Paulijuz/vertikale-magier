use crossbeam_channel::{unbounded, Receiver, Sender};
use log::warn;
use serde::{de, Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    io::{ErrorKind, Read, Result},
    net::{Ipv4Addr, Shutdown, SocketAddrV4},
    thread::{spawn, JoinHandle},
};

use crate::network::elevator_monitor::Heartbeat;

const BUFFER_SIZE: usize = 1024;

pub trait SendableType: Serialize + de::DeserializeOwned + Send + 'static {}
impl<T: Serialize + de::DeserializeOwned + Send + 'static> SendableType for T {}

pub struct Client<T: SendableType> {
    socket: Socket,
    send_channel: Option<Sender<T>>,
    receive_channel: Receiver<(SocketAddrV4, T)>,
    send_thread: Option<JoinHandle<()>>,
    receive_thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReceiveType<T> {
    Data(T),
    Heartbeat(Heartbeat),
}

fn receive<T: SendableType>(mut socket: Socket, receive_channel_tx: Sender<(SocketAddrV4, T)>) {
    loop {
        let mut buffer = [0; BUFFER_SIZE];

        let (Ok(address), Ok(count)) = (socket.peek_sender(), socket.read(&mut buffer)) else {
            break;
        };

        if count == 0 {
            break;
        }

        let address = address
            .as_socket_ipv4()
            .unwrap_or(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));
        let data: std::result::Result<ReceiveType<T>, _> = serde_json::from_slice(&buffer[..count]);

        match data {
            Ok(ReceiveType::Data(data)) => {
             
                receive_channel_tx.send((address, data)).unwrap();
            }
            Ok(ReceiveType::Heartbeat(heartbeat)) => {
                println!("Received heartbeat: {:?}", heartbeat);
                // utfør heartbeat funksjon og sjekk om heis er i live.
            }
            Err(_) => {
                warn!("Could not deserialize received data!");
            }
        }
    }
}

fn send<T: SendableType>(socket: Socket, send_channel_rx: Receiver<T>, send_address: SocketAddrV4) {
    loop {
        let Ok(data) = send_channel_rx.recv() else {
            break;
        };

        let Ok(buffer) = serde_json::to_vec(&data) else {
            panic!("Could not serialize data!");
        };

        socket.send_to(&buffer, &send_address.into()).unwrap();
    }
}

impl<T: SendableType> Client<T> {
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

impl<T: SendableType> Drop for Client<T> {
    fn drop(&mut self) {
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
    }
}
