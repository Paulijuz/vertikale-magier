use std::thread::{sleep, spawn};
use std::time::Duration;

use crossbeam_channel as cbc;

#[derive(Debug, Clone)]
pub struct Timer {
    timeout_channel_tx: cbc::Sender<()>,
    timeout_channel_rx: cbc::Receiver<()>,
}

impl Timer {
    pub fn init() -> Timer {
        let (timeout_channel_tx, timeout_channel_rx) = cbc::unbounded::<()>();

        Timer {
            timeout_channel_rx,
            timeout_channel_tx,
        }
    }

    pub fn start(&self, duration: Duration) {
        let timeout_channel_tx = self.timeout_channel_tx.clone();

        spawn(move || {
            sleep(duration);
            timeout_channel_tx.send(()).unwrap();
        });
    }

    pub fn trigger(&self) {
        self.timeout_channel_tx.send(()).unwrap();
    }

    pub fn timeout_channel(&self) -> &cbc::Receiver<()> {
        &self.timeout_channel_rx
    }
}
