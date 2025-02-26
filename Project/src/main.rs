use crossbeam_channel as cbc;
use driver_rust::elevio;
use elevator_controller::controller_loop;
use env_logger;
use log::{error, info, LevelFilter};
use request_dispatch::{start_master_server, start_slave_client};
use std::{env, process::exit, thread::spawn};
use clap::Parser;

mod elevator_controller;
mod hall_request_assigner;
mod inputs;
mod light_sync;
mod network;
mod request_dispatch;
mod timer;
mod backup;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, short)]
    name: Option<String>,

    #[arg(long, short, default_value_t = 15657)]
    port: u16,

    #[arg(long, short, default_value_t = false)]
    master: bool,

    #[arg(long, short, default_value_t = false)]
    slave: bool,
}

fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Trace)
        .init();

    let args = Args::parse();

    info!("Bruker port: {}", args.port);

    if args.master {
        start_master_server();
        return;
    }

    if args.slave{
        let elevio_driver: elevio::elev::Elevator =
            elevio::elev::Elevator::init(&format!("localhost:{}", args.port), 4).unwrap();

        let (command_channel_tx, command_channel_rx) = cbc::unbounded();
        let (elevator_event_tx, elevator_event_rx) = cbc::unbounded();

        {
            let elevio_driver = elevio_driver.clone();
            spawn(move || controller_loop(&elevio_driver, command_channel_rx, elevator_event_tx));
        }

        start_slave_client(None, &elevio_driver, command_channel_tx, elevator_event_rx);
        return;
    }
    
    error!("Programmet må startes som enten master eller slave. Kjør 'cargo run master' for master eller 'cargo run slave' for slave.");
    exit(1);
}
