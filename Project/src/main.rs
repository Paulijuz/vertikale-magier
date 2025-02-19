mod elevator_controller;
mod inputs;
mod light_sync;
mod network;
mod order_dispatch_new;
mod timer;

use order_dispatch_new::start_master_server;
use order_dispatch_new::start_slave_server;
use std::env;
use env_logger;
use log::info;
use log::LevelFilter;

fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Trace)
        .init();

    info!("Program startup!");

    if env::args().any(|arg| arg == "master") {
        start_master_server();
    } else {
        start_slave_server();
    }
}