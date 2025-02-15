use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::{
    thread::{self, JoinHandle},
    time::Duration,
};

use super::socket;
use crate::timer::Timer;

const ADVERTISING_INTERVAL: Duration = Duration::from_secs(1);
// Use port 52052 and 239.0.0.52 for group 52 <3
const ADVERTISING_IP: [u8; 4] = [239, 0, 0, 52];
const ADVERTISING_PORT: u16 = 52052;

enum AdvertiserCommand {
    Start,
    Stop,
    Exit,
}

pub struct Advertiser {
    control_channel_tx: Sender<AdvertiserCommand>,
    receive_channel_rx: Receiver<String>,
    thread: Option<JoinHandle<()>>,
}

impl Advertiser {
    pub fn init(advertisment: &String) -> Self {
        let (control_channel_tx, control_channel_rx) = unbounded::<AdvertiserCommand>();
        let (receive_channel_tx, receive_channel_rx) = unbounded::<String>();

        let advertisment = advertisment.clone();
        let thread = Some(thread::spawn(move || {
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

    pub fn receive_channel(&self) -> &Receiver<String> {
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

fn run_advertiser(
    advertisment: String,
    control_channel_rx: Receiver<AdvertiserCommand>,
    receive_channel_tx: Sender<String>,
) {
    let client = socket::Client::new_multicast_client(ADVERTISING_IP, ADVERTISING_PORT);
    let timer = Timer::init();
    let mut advertising = false;

    loop {
        select! {
            recv(control_channel_rx) -> command => {
                match command.unwrap() {
                    AdvertiserCommand::Start if !advertising => {
                        advertising = true;
                        timer.trigger();
                    },
                    AdvertiserCommand::Stop => advertising = false,
                    AdvertiserCommand::Exit => break,
                    _ => {},
                }
            }
            recv(timer.timeout_channel()) -> _ => {
                if !advertising {
                    continue;
                }

                client.sender().send(advertisment.clone()).unwrap();
                timer.start(ADVERTISING_INTERVAL);
            }
            recv(client.receiver()) -> data => {
                let data = data.unwrap();
                receive_channel_tx.send(data).unwrap();
            }
        }
    }
}
