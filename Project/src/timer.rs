use crossbeam_channel as cbc;
use log::{debug, warn};
use std::{
    thread::{spawn, JoinHandle},
    time::{Duration, Instant},
};

const DEADLINE_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy)]
pub enum TimerCommand {
    Start,
    Stop,
    Exit,
}

#[derive(Debug)]
pub struct Timer {
    timeout_channel_rx: cbc::Receiver<()>,
    timer_command_channel_tx: cbc::Sender<TimerCommand>,
    thread_handle: Option<JoinHandle<()>>,
}

impl Timer {
    pub fn new(duration: Duration) -> Timer {
        let (timeout_channel_tx, timeout_channel_rx) = cbc::unbounded::<()>();
        let (timer_command_channel_tx, timer_command_channel_rx) = cbc::unbounded::<TimerCommand>();

        let thread_handle =
            spawn(move || timer_loop(duration, timer_command_channel_rx, timeout_channel_tx));

        Timer {
            timeout_channel_rx,
            timer_command_channel_tx,
            thread_handle: Some(thread_handle),
        }
    }

    /// Starts the timer.
    ///
    /// **Note:** Calling start on an already started timer will have no effect.
    pub fn start(&mut self) {
        self.timer_command_channel_tx
            .send(TimerCommand::Start)
            .unwrap();
    }

    /// Stops the timer if it is running. Has no effect otherwise.
    pub fn stop(&mut self) {
        self.timer_command_channel_tx
            .send(TimerCommand::Stop)
            .unwrap();
    }

    /// Forces the timer to begin counting down from the beginning.
    pub fn restart(&mut self) {
        self.stop();
        self.start();
    }

    /// The channel on which timeouts are sent.
    pub fn timeout_channel(&self) -> &cbc::Receiver<()> {
        &self.timeout_channel_rx
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        debug!("Shutting down timer...");

        self.timer_command_channel_tx
            .send(TimerCommand::Exit)
            .expect("Command channel end should not be dropped yet.");

        self.thread_handle
            .take()
            .expect("Timer thread handle should be available when dropping.")
            .join()
            .expect("Timer thread should join without errors.");

        debug!("Timer shut down.");
    }
}

fn timer_loop(
    duration: Duration,
    timer_command_channel_rx: cbc::Receiver<TimerCommand>,
    timeout_channel_tx: cbc::Sender<()>,
) {
    let ticker = cbc::tick(DEADLINE_POLL_INTERVAL);
    let mut deadline: Option<Instant> = None;

    loop {
        cbc::select! {
            recv(timer_command_channel_rx) -> command => {
                let command = command.expect("Command channel should exist as long as this thread runs.");

                match command {
                    TimerCommand::Start if deadline.is_none() => {
                        deadline = Some(Instant::now() + duration)
                    },
                    TimerCommand::Stop => {
                        deadline = None
                    },
                    _ => {},
                };
            },
            recv(ticker) -> _ => {
                let Some(deadline_instant) = deadline else {
                    continue;
                };

                if Instant::now() > deadline_instant {
                    deadline = None;
                    timeout_channel_tx.send(()).expect("Timeout channel receiver should exist as long as this thread runs.");
                }
            }
        }
    }
}
