use std::time::Duration;
use std::thread::{spawn, sleep};

use crossbeam_channel as cbc;

pub fn start_timer(duration: Duration, channel: &cbc::Sender<()>) {    
    let channel = channel.clone();
    
    spawn(move || {
        sleep(duration);
        let _ = channel.send(());
    });
}