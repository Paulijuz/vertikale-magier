use crossbeam_channel::{select, tick, unbounded, Receiver, Sender};
use log::debug;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Result,
    net::SocketAddrV4,
    thread::{spawn, JoinHandle},
    time::{Duration, Instant},
};

use super::client::{Client, Transmit};

const ADVERTISING_INTERVAL: Duration = Duration::from_millis(100);
const ADVERTISMENT_LINGER: Duration = Duration::from_millis(5000);
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

pub struct Advertiser<T: Transmit + PartialEq + Eq> {
    control_channel_tx: Sender<AdvertiserCommand<T>>,
    receive_channel_rx: Receiver<HashMap<SocketAddrV4, T>>,
    thread: Option<JoinHandle<()>>,
}

fn generate_sender_id() -> [u8; ADVERTISER_ID_LENGTH] {
    let mut buffer = [0; ADVERTISER_ID_LENGTH];
    rand::rng().fill_bytes(&mut buffer);
    return buffer;
}

fn run_advertiser<T: Transmit + PartialEq + Eq>(
    data: Option<T>,
    client: Client<Advertisment<T>>,
    control_channel_rx: Receiver<AdvertiserCommand<T>>,
    receive_channel_tx: Sender<HashMap<SocketAddrV4, T>>,
) {
    let sender_id = generate_sender_id();
    let mut advertisment = data.map(|data| Advertisment { data, sender_id });
    let mut received_advertisments: HashMap<SocketAddrV4, (Instant, T)> = HashMap::new();
    let mut is_advertising = false;

    let ticker = tick(ADVERTISING_INTERVAL);

    loop {
        select! {
            recv(control_channel_rx) -> command => {
                match command.unwrap() {
                    AdvertiserCommand::Start => is_advertising = true,
                    AdvertiserCommand::Stop => is_advertising = false,
                    AdvertiserCommand::SetAdvertisment(data) => advertisment = Some(Advertisment { data, sender_id }),
                    AdvertiserCommand::Exit => break,
                }
            },
            recv(ticker) -> _ => {
                if !is_advertising {
                    continue;
                }

                if let Some(advertisment) = &advertisment {
                    client.send_channel().send(advertisment.clone()).expect("The advertiser's client should always be able to send.");
                }
            },
            recv(client.receive_channel()) -> data => {
                let (address, received_advertisment) = data.unwrap();

                if received_advertisment.sender_id == sender_id {
                    continue;
                }

                received_advertisments.insert(address, (Instant::now(), received_advertisment.data));
                received_advertisments.retain(|_, (instant_received, _)| instant_received.elapsed() < ADVERTISMENT_LINGER);

                let received_advertisment_data: HashMap<SocketAddrV4, T> = received_advertisments
                    .iter()
                    .map(|(addr, (_, data))| (addr.clone(), data.clone()))
                    .collect();

                receive_channel_tx.send(received_advertisment_data).unwrap();
            },
        }
    }
}

impl<T: Transmit + PartialEq + Eq> Advertiser<T> {
    pub fn new(advertisment: Option<T>, multicast_ip: [u8; 4], port: u16) -> Result<Self> {
        let client: Client<Advertisment<T>> = Client::new_udp_multicast_client(multicast_ip, port)?;

        let (control_channel_tx, control_channel_rx) = unbounded::<AdvertiserCommand<T>>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<HashMap<SocketAddrV4, T>>();

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

    pub fn receive_channel(&self) -> &Receiver<HashMap<SocketAddrV4, T>> {
        &self.receive_channel_rx
    }
}

impl<T: Transmit + PartialEq + Eq> Drop for Advertiser<T> {
    fn drop(&mut self) {
        debug!("Shutting down advertiser...");
        self.control_channel_tx
            .send(AdvertiserCommand::Exit)
            .unwrap();
        self.thread.take().unwrap().join().unwrap();
        debug!("Advertiser shut down.")
    }
}
