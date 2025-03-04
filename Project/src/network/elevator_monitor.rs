use super::client::{Client, SendableType};
use serde::{Deserialize, Serialize};
use std::net::SocketAddrV4;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub timestamp: u64, 
}

impl Heartbeat {
    pub fn new() -> Self {
        let now = Instant::now().elapsed().as_millis() as u64;
        Heartbeat { timestamp: now }
    }
}


pub fn send_heartbeat(
    client: &Client<Heartbeat>,
    heartbeat: Heartbeat,
) {
    client.send_channel().send( heartbeat).unwrap();
}

pub fn check_last_received(last_received: u64) {
    let now = Instant::now().elapsed().as_millis() as u64;
    if now - last_received > 500 {
        println!("Its dead")
    } 
}

pub fn start_heartbeat(client: &Client<Heartbeat>) {
    loop {
        let heartbeat = Heartbeat::new();
        send_heartbeat(client, heartbeat);
        thread::sleep(Duration::from_millis(15));
    }
}