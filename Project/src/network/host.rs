use crossbeam_channel::{select, tick, unbounded, Receiver, Sender};
use log::warn;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    io::Result,
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4},
    thread::{spawn, JoinHandle},
    time::Duration,
};

use super::client::{Client, SendableType};

const BACKLOG_SIZE: i32 = 128;
const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub const ALL_CLIENTS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);

pub struct Host<T: SendableType> {
    socket: Socket,
    send_channel: Option<Sender<(SocketAddrV4, T)>>,
    receive_channel: Receiver<(SocketAddrV4, T)>,
    accept_thread_handle: Option<JoinHandle<()>>,
    serve_thread_handle: Option<JoinHandle<()>>,
}

fn accept_clients<T: SendableType>(
    socket: Socket,
    new_client_channel_tx: Sender<(SocketAddrV4, Client<T>)>,
) {
    loop {
        let Ok((client_socket, client_address)) = socket.accept() else {
            break;
        };

        let client_address = client_address.as_socket_ipv4().unwrap();
        let client = Client::new(client_socket, client_address.clone()).unwrap();

        new_client_channel_tx
            .send((client_address, client))
            .unwrap();
    }
}

fn serve_clients<T: SendableType>(
    new_client_channel_rx: Receiver<(SocketAddrV4, Client<T>)>,
    send_channel_rx: Receiver<(SocketAddrV4, T)>,
    receive_channel_tx: Sender<(SocketAddrV4, T)>,
) {
    let mut clients: HashMap<SocketAddrV4, Client<T>> = HashMap::new();
    let ticker = tick(RECEIVE_POLL_INTERVAL);

    loop {
        select! {
            recv(new_client_channel_rx) -> new_client => {
                let Ok((address, client)) = new_client else { break; };

                clients.insert(address, client);
            },
            recv(send_channel_rx) -> message => {
                let Ok((address, data)) = message else { break; };

                if address == ALL_CLIENTS {
                    for client in clients.values() {
                        client.send_channel().send(data.clone()).unwrap();
                    }
                    continue;
                }

                let Some(client) = &clients.get(&address) else {
                    warn!("Warning: Tried sending to an unconnected address");
                    continue;
                };

                client.send_channel().send(data).unwrap();
            }
            recv(ticker) -> _ => {
                for (address, client) in &clients {
                    let Ok((_, data)) = client.receive_channel().try_recv() else { continue; };
                    receive_channel_tx.send((*address, data)).unwrap();
                }
            }
        }
    }
}

impl<T: SendableType> Host<T> {
    pub fn new_tcp_host(port: u16) -> Result<Self> {
        let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_reuse_address(true)?;
        socket.bind(&address.into())?;
        socket.listen(BACKLOG_SIZE)?;

        let (new_client_channel_tx, new_client_channel_rx) =
            unbounded::<(SocketAddrV4, Client<T>)>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();
        let (send_channel_tx, send_channel_rx) = unbounded::<(SocketAddrV4, T)>();

        let accept_socket = socket.try_clone()?;
        let accept_thread_handle: JoinHandle<()> = spawn(move || {
            accept_clients(accept_socket, new_client_channel_tx);
        });
        let serve_thread_handle = spawn(move || {
            serve_clients(new_client_channel_rx, send_channel_rx, receive_channel_tx);
        });

        Ok(Host {
            socket,
            send_channel: Some(send_channel_tx),
            receive_channel: receive_channel_rx,
            accept_thread_handle: Some(accept_thread_handle),
            serve_thread_handle: Some(serve_thread_handle),
        })
    }
    pub fn send_channel(&self) -> &Sender<(SocketAddrV4, T)> {
        self.send_channel
            .as_ref()
            .expect("Send channel should exist as long as host exists.")
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
