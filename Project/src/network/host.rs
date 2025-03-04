use crossbeam_channel::{select, unbounded, Receiver, Sender};
use log::warn;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4},
    thread::{spawn, JoinHandle},
    time::Duration,
};

use super::client::{Client, SendableType};

const BACKLOG_SIZE: i32 = 128;
const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct Host<T: SendableType> {
    socket: Socket,
    send_channel: Option<Sender<(SocketAddrV4, T)>>,
    receive_channel: Receiver<(SocketAddrV4, T)>,
    accept_thread_handle: Option<JoinHandle<()>>,
    serve_thread_handle: Option<JoinHandle<()>>,
}

fn start_accept_thread<T: SendableType>(
    socket: Socket,
    new_client_channel_tx: Sender<(SocketAddrV4, Client<T>)>,
) -> JoinHandle<()> {
    spawn(move || loop {
        let Ok((client_socket, client_address)) = socket.accept() else {
            break;
        };

        let client_address = client_address.as_socket_ipv4().unwrap();
        let clients = Client::new(client_socket, &client_address);

        new_client_channel_tx
            .send((client_address, clients))
            .unwrap();
    })
}

fn start_serve_thread<T: SendableType>(
    new_client_channel_rx: Receiver<(SocketAddrV4, Client<T>)>,
    send_channel_rx: Receiver<(SocketAddrV4, T)>,
    receive_channel_tx: Sender<(SocketAddrV4, T)>,
) -> JoinHandle<()> {
    spawn(move || {
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
                    client.send_channel().send(data).unwrap();
                }
                default(RECEIVE_POLL_INTERVAL) => {
                    for (address, client) in &clients {
                        let Ok((_, data)) = client.receive_channel().try_recv() else { continue; };
                        receive_channel_tx.send((*address, data)).unwrap();
                    }
                }
            }
        }
    })
}

impl<T: SendableType> Host<T> {
    pub fn new_tcp_host(port: Option<u16>) -> Self {
        let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port.unwrap_or(0)));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.bind(&address.into()).unwrap();
        socket.listen(BACKLOG_SIZE).unwrap();

        let (new_client_channel_tx, new_client_channel_rx) =
            unbounded::<(SocketAddrV4, Client<T>)>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<(SocketAddrV4, T)>();

        let accept_thread_handle =
            start_accept_thread(socket.try_clone().unwrap(), new_client_channel_tx);
        let serve_thread_handle =
            start_serve_thread(new_client_channel_rx, send_channel_rx, receive_channel_tx);

        Host {
            socket,
            send_channel: Some(send_channel_tx),
            receive_channel: receive_channel_rx,
            accept_thread_handle: Some(accept_thread_handle),
            serve_thread_handle: Some(serve_thread_handle),
        }
    }
    pub fn send_channel(&self) -> &Sender<(SocketAddrV4, T)> {
        self.send_channel.as_ref().unwrap()
    }
    pub fn receive_channel(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receive_channel
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
        drop(self.send_channel.take().unwrap());

        self.accept_thread_handle.take().unwrap().join().unwrap();
        self.serve_thread_handle.take().unwrap().join().unwrap();
    }
}
