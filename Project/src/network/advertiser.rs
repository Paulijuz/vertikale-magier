use super::socket::{Client, SendableType};
use crate::timer::Timer;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use rand::RngCore;
use serde::{ Deserialize, Serialize};
use std::{
    net::SocketAddrV4,
    thread::{spawn, JoinHandle},
    time::Duration,
    u8,
};

const ADVERTISING_INTERVAL: Duration = Duration::from_secs(1);
// Use port 52052 and 239.0.0.52 for group 52 <3
const ADVERTISING_IP: [u8; 4] = [239, 0, 0, 52];
const ADVERTISING_PORT: u16 = 52052;
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

pub struct Advertiser<T: SendableType + Clone> {
    control_channel_tx: Sender<AdvertiserCommand<T>>,
    receive_channel_rx: Receiver<(SocketAddrV4, T)>,
    thread: Option<JoinHandle<()>>,
}

impl<T: SendableType + Clone> Advertiser<T> {
    pub fn init(advertisment: T) -> Self {
        let (control_channel_tx, control_channel_rx) = unbounded::<AdvertiserCommand<T>>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, T)>();

        let thread = Some(spawn(move || {
            run_advertiser(advertisment, control_channel_rx, receive_channel_tx)
        }));

        Advertiser {
            control_channel_tx,
            receive_channel_rx,
            thread,
        }
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

impl<T: SendableType + Clone> Drop for Advertiser<T> {
    fn drop(&mut self) {
        self.control_channel_tx
            .send(AdvertiserCommand::Exit)
            .unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}

fn generate_advertiser_id() -> [u8; ADVERTISER_ID_LENGTH] {
    let mut buffer = [0; ADVERTISER_ID_LENGTH];
    rand::rng().fill_bytes(&mut buffer);
    return buffer;
}

fn run_advertiser<T: SendableType + Clone>(
    advertisment_data: T,
    control_channel_rx: Receiver<AdvertiserCommand<T>>,
    receive_channel_tx: Sender<(SocketAddrV4, T)>,
) {
    let mut advertisment = Advertisment {
        sender_id: generate_advertiser_id(),
        data: advertisment_data,
    };

    let client: Client<Advertisment<T>> =
        Client::new_multicast_client(ADVERTISING_IP, ADVERTISING_PORT);
    let mut timer = Timer::init();
    let mut is_advertising = false;

    loop {
        select! {
            recv(control_channel_rx) -> command => {
                match command.unwrap() {
                    AdvertiserCommand::Start => {
                        if is_advertising {
                            continue;
                        }

                        is_advertising = true;
                        timer.start(ADVERTISING_INTERVAL);
                    },
                    AdvertiserCommand::Stop => is_advertising = false,
                    AdvertiserCommand::SetAdvertisment(new_advertisment_data) => {
                        advertisment = Advertisment {
                            sender_id: generate_advertiser_id(),
                            data: new_advertisment_data,
                        };
                    },
                    AdvertiserCommand::Exit => break,
                }
            },
            recv(timer.timeout_channel()) -> _ => {
                if !is_advertising {
                    continue;
                }

                client.sender().send(advertisment.clone()).unwrap();
                timer.start(ADVERTISING_INTERVAL);
            },
            recv(client.receiver()) -> data => {
                let (address, received_advertisment) = data.unwrap();

                if received_advertisment.sender_id == advertisment.sender_id {
                    continue;
                }

                receive_channel_tx.send((address, received_advertisment.data)).unwrap();
            },
        }
    }
}
