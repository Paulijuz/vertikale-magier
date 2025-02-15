use crossbeam_channel::{select, unbounded, Receiver, Sender};
use log::warn;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read},
    net::{Ipv4Addr, Shutdown, SocketAddr},
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

const BUFFER_SIZE: usize = 1024;
const BACKLOG_SIZE: i32 = 128;

pub struct Client {
    socket: Socket,
    sender: Option<Sender<String>>,
    receiver: Receiver<(SockAddr, String)>,
    sender_thread: Option<JoinHandle<()>>,
    receiver_thread: Option<JoinHandle<()>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.socket
            .shutdown(Shutdown::Both)
            .unwrap_or_else(|error| {
                if error.kind() != ErrorKind::NotConnected {
                    panic!("{error:?}");
                }
            });
        drop(self.sender.take().unwrap());

        self.sender_thread.take().unwrap().join().unwrap();
        self.receiver_thread.take().unwrap().join().unwrap();
    }
}

impl Client {
    fn init(socket: Socket, send_address: &SockAddr) -> Self {
        let mut receive_socket = socket.try_clone().unwrap();
        let send_socket: Socket = socket.try_clone().unwrap();

        let send_address = send_address.clone();

        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SockAddr, String)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<String>();

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

            let data = String::from_utf8_lossy(&buffer[..count]);
            receive_channel_tx.send((address, data.into())).unwrap();
        });

        let send_thread_handle = spawn(move || loop {
            let Ok(data) = send_channel_rx.recv() else {
                break;
            };
            send_socket.send_to(data.as_bytes(), &send_address).unwrap();
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
        let address = SocketAddr::from((multicast_ip, port));

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.bind(&address.into()).unwrap();
        socket
            .join_multicast_v4(&multicast_ip, &Ipv4Addr::UNSPECIFIED)
            .unwrap();

        Client::init(socket, &address.into())
    }
    pub fn new_tcp_client(host_ip: [u8; 4], port: u16) -> Self {
        let address: SocketAddr = SocketAddr::from((host_ip, port));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.connect(&address.into()).unwrap();

        Client::init(socket, &address.into())
    }
    pub fn sender(&self) -> &Sender<String> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(SockAddr, String)> {
        &self.receiver
    }
}

pub struct Host {
    socket: Socket,
    sender: Option<Sender<(u32, String)>>,
    receiver: Receiver<(u32, String)>,
    accept_thread_handle: Option<JoinHandle<()>>,
    serve_thread_handle: Option<JoinHandle<()>>,
}

impl Host {
    pub fn new_tcp_host(port: Option<u16>) -> Self {
        let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port.unwrap_or(0)));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.bind(&address.into()).unwrap();
        socket.listen(BACKLOG_SIZE).unwrap();

        let (new_client_channel_tx, new_client_channel_rx) = unbounded::<(SockAddr, Client)>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(u32, String)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<(u32, String)>();

        let accept_socket = socket.try_clone().unwrap();
        let accept_thread_handle = spawn(move || loop {
            let Ok((client_socket, client_address)) = accept_socket.accept() else {
                break;
            };

            let clients = Client::init(client_socket, &client_address);

            new_client_channel_tx
                .send((client_address, clients))
                .unwrap();
        });

        let serve_thread_handle = spawn(move || {
            let mut client_ids: HashMap<SockAddr, u32> = HashMap::new();
            let mut clients: HashMap<u32, Client> = HashMap::new();

            loop {
                select! {
                    recv(new_client_channel_rx) -> new_client => {
                        let Ok((address, client)) = new_client else { break; };

                        if !client_ids.contains_key(&address) {
                            client_ids.insert(address.clone(), client_ids.len() as u32);
                        }

                        clients.insert(client_ids[&address], client);
                    },
                    recv(send_channel_rx) -> message => {
                        let Ok((id, data)) = message else { break; };
                        let Some(client) = &clients.get(&id) else {
                            warn!("Warning: Tried sending to a non existent id");
                            continue;
                        };
                        client.sender().send(data).unwrap();
                    }
                    default => {
                        for (id, client) in &clients {
                            let Ok((_, data)) = client.receiver().try_recv() else { continue; };
                            receive_channel_tx.send((*id, data)).unwrap();
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
    pub fn sender(&self) -> &Sender<(u32, String)> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(u32, String)> {
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

impl Drop for Host {
    fn drop(&mut self) {
        self.socket.shutdown(Shutdown::Both).unwrap();
        drop(self.sender.take().unwrap());

        self.accept_thread_handle.take().unwrap().join().unwrap();
        self.serve_thread_handle.take().unwrap().join().unwrap();
    }
}
