use crossbeam_channel::{unbounded, Receiver, Sender};
use log::warn;
use serde::{de, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    io::{ErrorKind, Read, Result},
    net::{Ipv4Addr, Shutdown, SocketAddrV4},
    thread::{spawn, JoinHandle},
};

const BUFFER_SIZE: usize = 1024;

pub trait SendableType: Serialize + de::DeserializeOwned + Send + 'static {}
impl<T: Serialize + de::DeserializeOwned + Send + 'static> SendableType for T {}

pub struct Client<T: SendableType> {
    socket: Socket,
    sender: Option<Sender<T>>,
    receiver: Receiver<(SocketAddrV4, T)>,
    sender_thread: Option<JoinHandle<()>>,
    receiver_thread: Option<JoinHandle<()>>,
}

impl<T: SendableType> Client<T> {
    pub fn new(socket: Socket, send_address: &SocketAddrV4) -> Self {
        let mut receive_socket = socket.try_clone().unwrap();
        let send_socket = socket.try_clone().unwrap();

        let send_address = send_address.to_owned();

        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<T>();

        let receive_thread_handle = spawn(move || loop {
            let mut buffer = [0; BUFFER_SIZE];

            let (Ok(address), Ok(count)) = (
                receive_socket.peek_sender(),
                receive_socket.read(&mut buffer),
            ) else {
                break;
            };

            if count == 0 {
                break;
            }

            let address = address
                .as_socket_ipv4()
                .unwrap_or(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));
            let Ok(data) = serde_json::from_slice::<T>(&buffer[..count]) else {
                warn!("Could not deserialize received data!");
                continue;
            };

            receive_channel_tx.send((address, data.into())).unwrap();
        });

        let send_thread_handle = spawn(move || loop {
            let Ok(data) = send_channel_rx.recv() else {
                break;
            };

            let Ok(buffer) = serde_json::to_vec(&data) else {
                panic!("Could not serialize data!");
            };

            send_socket.send_to(&buffer, &send_address.into()).unwrap();
        });

        Client {
            socket,
            sender: Some(send_channel_tx),
            receiver: receive_channel_rx,
            sender_thread: Some(send_thread_handle),
            receiver_thread: Some(receive_thread_handle),
        }
    }
    pub fn new_udp_multicast_client(multicast_ip: [u8; 4], port: u16) -> Self {
        let multicast_ip = Ipv4Addr::from(multicast_ip);
        let address = SocketAddrV4::new(multicast_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.bind(&address.into()).unwrap();
        socket
            .join_multicast_v4(&multicast_ip, &Ipv4Addr::UNSPECIFIED)
            .unwrap();

        Client::new(socket, &address)
    }
    pub fn new_tcp_client(host_ip: [u8; 4], port: u16) -> Result<Self> {
        let host_ip = Ipv4Addr::from(host_ip);
        let address = SocketAddrV4::new(host_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.connect(&address.into())?;

        Ok(Client::new(socket, &address))
    }
    pub fn send_channel(&self) -> &Sender<T> {
        self.sender.as_ref().unwrap()
    }
    pub fn receive_channel(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receiver
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
        drop(self.sender.take().unwrap());

        self.sender_thread.take().unwrap().join().unwrap();
        self.receiver_thread.take().unwrap().join().unwrap();
    }
}
