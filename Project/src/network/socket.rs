use crossbeam_channel::{select, unbounded, Receiver, Sender};
use log::warn;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read},
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4},
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

const BUFFER_SIZE: usize = 1024;
const BACKLOG_SIZE: i32 = 128;

pub struct Client {
    socket: Socket,
    sender: Option<Sender<String>>,
    receiver: Receiver<(SocketAddrV4, String)>,
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
    fn init(socket: Socket, send_address: &SocketAddrV4) -> Self {
        let mut receive_socket = socket.try_clone().unwrap();
        let send_socket: Socket = socket.try_clone().unwrap();

        let send_address = send_address.clone();

        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, String)>();
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

            let address = address.as_socket_ipv4().unwrap();
            let data = String::from_utf8_lossy(&buffer[..count]);

            receive_channel_tx.send((address, data.into())).unwrap();
        });

        let send_thread_handle = spawn(move || loop {
            let Ok(data) = send_channel_rx.recv() else {
                break;
            };
            send_socket
                .send_to(data.as_bytes(), &send_address.into())
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
    pub fn new_tcp_client(host_ip: [u8; 4], port: u16) -> Self {
        let host_ip = Ipv4Addr::from(host_ip);
        let address = SocketAddrV4::new(host_ip, port);

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.connect(&address.into()).unwrap(); // TODO: Gracefully handle!

        Client::init(socket, &address)
    }
    pub fn sender(&self) -> &Sender<String> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(SocketAddrV4, String)> {
        &self.receiver
    }
}

pub struct Host {
    socket: Socket,
    sender: Option<Sender<(SocketAddrV4, String)>>,
    receiver: Receiver<(SocketAddrV4, String)>,
    accept_thread_handle: Option<JoinHandle<()>>,
    serve_thread_handle: Option<JoinHandle<()>>,
}

impl Host {
    pub fn new_tcp_host(port: Option<u16>) -> Self {
        let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port.unwrap_or(0)));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.bind(&address.into()).unwrap();
        socket.listen(BACKLOG_SIZE).unwrap();

        let (new_client_channel_tx, new_client_channel_rx) = unbounded::<(SocketAddrV4, Client)>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, String)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<(SocketAddrV4, String)>();

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
            let mut clients: HashMap<SocketAddrV4, Client> = HashMap::new();

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
    pub fn sender(&self) -> &Sender<(SocketAddrV4, String)> {
        self.sender.as_ref().unwrap()
    }
    pub fn receiver(&self) -> &Receiver<(SocketAddrV4, String)> {
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
