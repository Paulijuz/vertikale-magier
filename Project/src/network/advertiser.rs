use crossbeam_channel::{select, tick, unbounded, Receiver, Sender};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    io::Result,
    net::SocketAddrV4,
    thread::{spawn, JoinHandle},
    time::Duration,
};

use super::client::{Client, SendableType};

const ADVERTISING_INTERVAL: Duration = Duration::from_millis(1000);
const ADVERTISER_ID_LENGTH: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advertisment<T: Clone> {
    sender_id: [u8; ADVERTISER_ID_LENGTH],
    data: T,
}

enum AdvertiserCommand<T> {
    Start,
    Stop,
    SetAdvertisment(T),
    Exit,
}

pub struct Advertiser<T: SendableType> {
    control_channel_tx: Sender<AdvertiserCommand<T>>,
    receive_channel_rx: Receiver<(SocketAddrV4, T)>,
    thread: Option<JoinHandle<()>>,
}

fn generate_sender_id() -> [u8; ADVERTISER_ID_LENGTH] {
    let mut buffer = [0; ADVERTISER_ID_LENGTH];
    rand::rng().fill_bytes(&mut buffer);
    return buffer;
}

fn run_advertiser<T: SendableType>(
    data: T,
    client: Client<Advertisment<T>>,
    control_channel_rx: Receiver<AdvertiserCommand<T>>,
    receive_channel_tx: Sender<(SocketAddrV4, T)>,
) {
    let mut advertisment = Advertisment {
        sender_id: generate_sender_id(),
        data,
    };
    let mut is_advertising = false;

    let ticker = tick(ADVERTISING_INTERVAL);

    loop {
        select! {
            recv(control_channel_rx) -> command => {
                match command.unwrap() {
                    AdvertiserCommand::Start => is_advertising = true,
                    AdvertiserCommand::Stop => is_advertising = false,
                    AdvertiserCommand::SetAdvertisment(data) => advertisment.data = data,
                    AdvertiserCommand::Exit => break,
                }
            },
            recv(ticker) -> _ => {
                if !is_advertising {
                    continue;
                }

                client.send_channel().send(advertisment.clone()).unwrap();
            },
            recv(client.receive_channel()) -> data => {
                let (address, received_advertisment) = data.unwrap();

                if received_advertisment.sender_id == advertisment.sender_id {
                    continue;
                }

                receive_channel_tx.send((address, received_advertisment.data)).unwrap();
            },
        }
    }
}

impl<T: SendableType> Advertiser<T> {
    pub fn new(advertisment: T, multicast_ip: [u8; 4], port: u16) -> Result<Self> {
        let client: Client<Advertisment<T>> = Client::new_udp_multicast_client(multicast_ip, port)?;

        let (control_channel_tx, control_channel_rx) = unbounded::<AdvertiserCommand<T>>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();

        let thread = spawn(move || {
            run_advertiser(advertisment, client, control_channel_rx, receive_channel_tx)
        });

        Ok(Advertiser {
            control_channel_tx,
            receive_channel_rx,
            thread: Some(thread),
        })
    }

    pub fn start_advertising(&self) {
        self.control_channel_tx
            .send(AdvertiserCommand::Start)
            .unwrap();
    }

    pub fn stop_advertising(&self) {
        self.control_channel_tx
            .send(AdvertiserCommand::Stop)
            .unwrap();
    }

    pub fn set_advertisment(&self, advertisment: T) {
        self.control_channel_tx
            .send(AdvertiserCommand::SetAdvertisment(advertisment))
            .unwrap();
    }

    pub fn receive_channel(&self) -> &Receiver<(SocketAddrV4, T)> {
        &self.receive_channel_rx
    }
}

impl<T: SendableType> Drop for Advertiser<T> {
    fn drop(&mut self) {
        self.control_channel_tx
            .send(AdvertiserCommand::Exit)
            .unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}
