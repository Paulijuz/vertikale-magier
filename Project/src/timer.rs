use crossbeam_channel as cbc;
use std::ops::Add;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Timer {
    timeout_channel_tx: cbc::Sender<()>,
    timeout_channel_rx: cbc::Receiver<()>,
    duration: Duration,
    is_active: Arc<AtomicBool>,
    iteration: Arc<AtomicU32>,
}

impl Timer {
    pub fn new(duration: Duration) -> Timer {
        let (timeout_channel_tx, timeout_channel_rx) = cbc::unbounded::<()>();

        Timer {
            timeout_channel_rx,
            timeout_channel_tx,
            duration,
            is_active: Arc::new(AtomicBool::new(false)),
            iteration: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn start(&mut self) {
        if self.is_active.fetch_or(true, Ordering::Relaxed) {
            return;
        }

        let timeout_channel_tx = self.timeout_channel_tx.clone();
        let duration = self.duration.clone();
        let is_active = Arc::clone(&self.is_active);
        let start_iteration: u32 = self.iteration.load(Ordering::Relaxed);
        let iteration = self.iteration.clone();

        spawn(move || {
            sleep(duration);
            
            if start_iteration == iteration.load(Ordering::Relaxed) {
                is_active.store(false, Ordering::Relaxed);
                timeout_channel_tx.send(()).unwrap();
            }
        });
    }

    pub fn stop(&mut self) {
        self.iteration.fetch_add(1, Ordering::Relaxed);
        self.is_active.store(false, Ordering::Relaxed);
    }

    pub fn restart(&mut self) {
        self.stop();
        self.start();
    }

    pub fn timeout_channel(&self) -> &cbc::Receiver<()> {
        &self.timeout_channel_rx
    }
}
