use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use crossbeam_channel as cbc;

#[derive(Debug)]
pub struct Timer {
    timeout_channel_tx: cbc::Sender<()>,
    timeout_channel_rx: cbc::Receiver<()>,
    is_active: Arc<AtomicBool>,
}

impl Timer {
    pub fn init() -> Timer {
        let (timeout_channel_tx, timeout_channel_rx) = cbc::unbounded::<()>();

        Timer {
            timeout_channel_rx,
            timeout_channel_tx,
            is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self, duration: Duration) {
        if self.is_active.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_err() {
            return;
        }

        let timeout_channel_tx = self.timeout_channel_tx.clone();
        let is_active = Arc::clone(&self.is_active);

        spawn(move || {
            sleep(duration);
            is_active.store(false, Ordering::Relaxed);
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
