mod elevator_controller;
mod hall_request_assigner;
mod inputs;
mod light_sync;
mod network;
mod request_dispatch;
mod timer;

use crossbeam_channel as cbc;
use driver_rust::elevio;
use elevator_controller::controller_loop;
use env_logger;
use log::{error, info, LevelFilter};
use request_dispatch::{start_master_server, start_slave_client};
use std::{env, thread::spawn};

fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Trace)
        .init();

    if env::args().any(|arg| arg == "master") {
        start_master_server();
        return;
    }

    if env::args().any(|arg| arg == "slave") {
        let elevio_driver: elevio::elev::Elevator =
            elevio::elev::Elevator::init("localhost:15657", 4).unwrap();

        let (command_channel_tx, command_channel_rx) = cbc::unbounded();
        let (elevator_event_tx, elevator_event_rx) = cbc::unbounded();

        {
            let elevio_driver = elevio_driver.clone();
            spawn(move || controller_loop(&elevio_driver, command_channel_rx, elevator_event_tx));
        }

        start_slave_client(&elevio_driver, command_channel_tx, elevator_event_rx);
        return;
    }

    error!("Programmet må startes som enten master eller slave. Kjør 'cargo run master' for master eller 'cargo run slave' for slave.")
}
