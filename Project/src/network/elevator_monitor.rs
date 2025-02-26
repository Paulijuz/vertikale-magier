use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
    thread::{spawn, JoinHandle},
};

// Check interval
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
// Time-out duration
const ELEVATOR_TIMEOUT: Duration = Duration::from_secs(10);
// ID-length
const ELEVATOR_ID_LENGTH: usize = 16;

// Struct for heartbeat message
#[derive(Debug, Clone)]
struct Heartbeat {
    elevator_id: [u8; ELEVATOR_ID_LENGTH], 
    timestamp: Instant, // Timestamp of the heartbeat message
}

// Struct to monitor status
pub struct ElevatorMonitor {
    heartbeat_tx: Sender<Heartbeat>, // Channel to send heartbeat messages
    thread: Option<JoinHandle<()>>, // Handle for the monitoring thread
}

impl ElevatorMonitor {
    // Creates a new ElevatorMonitor instance
    pub fn new() -> Self {
        // Create a channel for heartbeat messages
        let (heartbeat_tx, heartbeat_rx) = unbounded::<Heartbeat>();

        // Spawn a thread to run the elevator monitor
        let thread = Some(spawn(move || {
            run_elevator_monitor(heartbeat_rx)
        }));

        ElevatorMonitor {
            heartbeat_tx,
            thread,
        }
    }

    // Sends a heartbeat message for the given elevator ID
    pub fn send_heartbeat(&self, elevator_id: [u8; ELEVATOR_ID_LENGTH]) {
        self.heartbeat_tx.send(Heartbeat {
            elevator_id,
            timestamp: Instant::now(),
        }).unwrap();
    }
}

// Implement the Drop trait to ensure the monitoring thread is properly joined when the ElevatorMonitor is dropped
impl Drop for ElevatorMonitor {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().unwrap();
    }
}

// Function to run the elevator monitor
fn run_elevator_monitor(heartbeat_rx: Receiver<Heartbeat>) {
    // HashMap to store the last seen timestamp for each elevator
    let mut elevator_status: HashMap<[u8; ELEVATOR_ID_LENGTH], Instant> = HashMap::new();

    loop {
        select! {
            // Receive a heartbeat message
            recv(heartbeat_rx) -> heartbeat => {
                let heartbeat = heartbeat.unwrap();
                // Update the last seen timestamp for the elevator
                elevator_status.insert(heartbeat.elevator_id, heartbeat.timestamp);
            },
            // Check for timed out elevators at regular intervals
            default(HEARTBEAT_INTERVAL) => {
                let now = Instant::now();
                // Retain only the elevators that have sent a heartbeat within the timeout duration
                elevator_status.retain(|_, &mut last_seen| now.duration_since(last_seen) < ELEVATOR_TIMEOUT);

                // Iterate over the elevator statuses and print a message for any timed out elevators
                for (elevator_id, last_seen) in &elevator_status {
                    if now.duration_since(*last_seen) >= ELEVATOR_TIMEOUT {
                        println!("Elevator {:?} has timed out", elevator_id);
                    }
                }
            },
        }
    }
}