use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
    thread::{spawn, JoinHandle},
};


const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
// Time-out tid
const ELEVATOR_TIMEOUT: Duration = Duration::from_secs(10);
// ID-lengde
const ELEVATOR_ID_LENGTH: usize = 16;

//
#[derive(Debug, Clone)]
struct Heartbeat {
    elevator_id: [u8; ELEVATOR_ID_LENGTH], 
    timestamp: Instant, // Timestamp of the heartbeat message
}

// monitor status
pub struct ElevatorMonitor {
    heartbeat_tx: Sender<Heartbeat>, // Channel to send heartbeat messages
    thread: Option<JoinHandle<()>>, // Handle for the monitoring thread
}

impl ElevatorMonitor {
    // lage ny ElevatorMonitor instance
    pub fn new() -> Self {
        // kanal for meldinger
        let (heartbeat_tx, heartbeat_rx) = unbounded::<Heartbeat>();

        // lage ny thread
        let thread = Some(spawn(move || {
            run_elevator_monitor(heartbeat_rx)
        }));

        ElevatorMonitor {
            heartbeat_tx,
            thread,
        }
    }

    //Sende melding for en gitt heis
    pub fn send_heartbeat(&self, elevator_id: [u8; ELEVATOR_ID_LENGTH]) {
        self.heartbeat_tx.send(Heartbeat {
            elevator_id,
            timestamp: Instant::now(),
        }).unwrap();
    }
}

impl Drop for ElevatorMonitor {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().unwrap();
    }
}

// 
fn run_elevator_monitor(heartbeat_rx: Receiver<Heartbeat>) {
    // Hashmap for å lagre tidsstempel for siste melding fra hver heis
    let mut elevator_status: HashMap<[u8; ELEVATOR_ID_LENGTH], Instant> = HashMap::new();

    loop {
        select! {
            // Motta melding
            recv(heartbeat_rx) -> heartbeat => {
                let heartbeat = heartbeat.unwrap();
                // oppdatere tidsstempel for heisen
                elevator_status.insert(heartbeat.elevator_id, heartbeat.timestamp);
            },
            // Sjekk for timeout heiser.
            default(HEARTBEAT_INTERVAL) => {
                let now = Instant::now();
                // Behold kun heiser som ikke er timet out.
                elevator_status.retain(|_, &mut last_seen| now.duration_since(last_seen) < ELEVATOR_TIMEOUT);

            
                for (elevator_id, last_seen) in &elevator_status {
                    if now.duration_since(*last_seen) >= ELEVATOR_TIMEOUT {
                        println!("Elevator {:?} has timed out", elevator_id);
                    }
                }
            },
        }
    }
}