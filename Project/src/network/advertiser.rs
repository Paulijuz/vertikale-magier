use super::socket;
use crate::timer::Timer;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use rand::{distr::Alphanumeric, Rng};
use std::{
    net::SocketAddrV4,
    thread::{spawn, JoinHandle},
    time::Duration,
};

const ADVERTISING_INTERVAL: Duration = Duration::from_secs(1);
// Use port 52052 and 239.0.0.52 for group 52 <3
const ADVERTISING_IP: [u8; 4] = [239, 0, 0, 52];
const ADVERTISING_PORT: u16 = 52052;
const ADVERTISER_ID_LENGTH: usize = 16;

enum AdvertiserCommand {
    Start,
    Stop,
    Exit,
}

pub struct Advertiser {
    control_channel_tx: Sender<AdvertiserCommand>,
    receive_channel_rx: Receiver<(SocketAddrV4, String)>,
    thread: Option<JoinHandle<()>>,
}

impl Advertiser {
    pub fn init(advertisment: &String) -> Self {
        let (control_channel_tx, control_channel_rx) = unbounded::<AdvertiserCommand>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<(SocketAddrV4, String)>();

        let advertisment = advertisment.clone();
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

    pub fn receive_channel(&self) -> &Receiver<(SocketAddrV4, String)> {
        &self.receive_channel_rx
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        self.control_channel_tx
            .send(AdvertiserCommand::Exit)
            .unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}

fn generate_advertiser_id() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(ADVERTISER_ID_LENGTH)
        .map(char::from)
        .collect()
}

fn run_advertiser(
    mut advertisment: String,
    control_channel_rx: Receiver<AdvertiserCommand>,
    receive_channel_tx: Sender<(SocketAddrV4, String)>,
) {
    let id: String = generate_advertiser_id();
    advertisment.insert_str(0, &id);

    let client = socket::Client::new_multicast_client(ADVERTISING_IP, ADVERTISING_PORT);
    let timer = Timer::init();
    let mut is_advertising = false;

    loop {
        select! {
            recv(control_channel_rx) -> command => {
                match command.unwrap() {
                    AdvertiserCommand::Start if !is_advertising => {
                        is_advertising = true;
                        timer.trigger();
                    },
                    AdvertiserCommand::Stop => is_advertising = false,
                    AdvertiserCommand::Exit => break,
                    _ => {},
                }
            }
            recv(timer.timeout_channel()) -> _ => {
                if !is_advertising {
                    continue;
                }

                client.sender().send(advertisment.clone()).unwrap();
                timer.start(ADVERTISING_INTERVAL);
            }
            recv(client.receiver()) -> data => {
                let (address, data) = data.unwrap();

                let received_id = data[..ADVERTISER_ID_LENGTH].to_string();
                let received_advertisment = data[ADVERTISER_ID_LENGTH..].to_string();

                if received_id == id {
                    continue;
                }

                receive_channel_tx.send((address, received_advertisment)).unwrap();
            }
        }
    }
}
