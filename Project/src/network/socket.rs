use crossbeam_channel::{select, unbounded, Receiver, Sender};
use log::warn;
use serde::{de, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Result},
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4},
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

const BUFFER_SIZE: usize = 1024;
const BACKLOG_SIZE: i32 = 128;

pub trait SendableType: Serialize + de::DeserializeOwned + Send + 'static {}

impl<T: Serialize + de::DeserializeOwned + Send + 'static> SendableType for T {}

pub struct Client<T: SendableType> {
    socket: Socket,
    sender: Option<Sender<T>>,
    receiver: Receiver<(SocketAddrV4, T)>,
    sender_thread: Option<JoinHandle<()>>,
    receiver_thread: Option<JoinHandle<()>>,
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

impl<T: SendableType> Client<T> {
    fn init(socket: Socket, send_address: &SocketAddrV4) -> Self {
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

            let address = address.as_socket_ipv4().unwrap();
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

            send_socket
                .send_to(&buffer, &send_address.into())
                .unwrap();
        });

        Client {
            socket,
            sender: Some(send_channel_tx),
            receiver: receive_channel_rx,
            sender_thread: Some(send_thread_handle),
            receiver_thread: Some(receive_thread_handle),
        }
    }
    pub fn new_multicast_client(multicast_ip: [u8; 4], port: u16) -> Self {
        let multicast_ip = Ipv4Addr::from(multicast_ip);
        let address = SocketAddrV4::new(multicast_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.bind(&address.into()).unwrap();
        socket
            .join_multicast_v4(&multicast_ip, &Ipv4Addr::UNSPECIFIED)
            .unwrap();

        Client::init(socket, &address)
    }
    pub fn new_tcp_client(host_ip: [u8; 4], port: u16) -> Result<Self> {
        let host_ip = Ipv4Addr::from(host_ip);
        let address = SocketAddrV4::new(host_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.connect(&address.into())?;

        Ok(Client::init(socket, &address))
    }
    pub fn sender(&self) -> &Sender<T> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receiver
    }
}

pub struct Host<T: SendableType> {
    socket: Socket,
    sender: Option<Sender<(SocketAddrV4, T)>>,
    receiver: Receiver<(SocketAddrV4, T)>,
    accept_thread_handle: Option<JoinHandle<()>>,
    serve_thread_handle: Option<JoinHandle<()>>,
}

impl<T: SendableType> Host<T> {
    pub fn new_tcp_host(port: Option<u16>) -> Self {
        let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port.unwrap_or(0)));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.bind(&address.into()).unwrap();
        socket.listen(BACKLOG_SIZE).unwrap();

        let (new_client_channel_tx, new_client_channel_rx) = unbounded::<(SocketAddrV4, Client<T>)>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<(SocketAddrV4, T)>();

        let accept_socket: Socket = socket.try_clone().unwrap();
        let accept_thread_handle = spawn(move || loop {
            let Ok((client_socket, client_address)) = accept_socket.accept() else {
                break;
            };

            let client_address = client_address.as_socket_ipv4().unwrap();
            let clients = Client::init(client_socket, &client_address);

            new_client_channel_tx
                .send((client_address, clients))
                .unwrap();
        });

        let serve_thread_handle = spawn(move || {
            let mut clients: HashMap<SocketAddrV4, Client<T>> = HashMap::new();

            loop {
                select! {
                    recv(new_client_channel_rx) -> new_client => {
                        let Ok((address, client)) = new_client else { break; };

                        clients.insert(address, client);
                    },
                    recv(send_channel_rx) -> message => {
                        let Ok((address, data)) = message else { break; };
                        let Some(client) = &clients.get(&address) else {
                            warn!("Warning: Tried sending to an unconnected address");
                            continue;
                        };
                        client.sender().send(data).unwrap();
                    }
                    default => {
                        for (address, client) in &clients {
                            let Ok((_, data)) = client.receiver().try_recv() else { continue; };
                            receive_channel_tx.send((*address, data)).unwrap();
                        }
                        sleep(Duration::from_millis(10));
                    }
                }
            }
        });

        Host {
            socket,
            sender: Some(send_channel_tx),
            receiver: receive_channel_rx,
            accept_thread_handle: Some(accept_thread_handle),
            serve_thread_handle: Some(serve_thread_handle),
        }
    }
    pub fn sender(&self) -> &Sender<(SocketAddrV4, T)> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receiver
    }
    pub fn port(&self) -> u16 {
        self.socket
            .local_addr()
            .unwrap()
            .as_socket()
            .unwrap()
            .port()
    }
}

impl<T: SendableType> Drop for Host<T> {
    fn drop(&mut self) {
        self.socket.shutdown(Shutdown::Both).unwrap();
        drop(self.sender.take().unwrap());

        self.accept_thread_handle.take().unwrap().join().unwrap();
        self.serve_thread_handle.take().unwrap().join().unwrap();
    }
}
