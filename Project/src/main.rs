use backup::load_state_from_file;
use clap::Parser;
use crossbeam_channel as cbc;
use driver_rust::elevio;
use elevator::controller::controller_loop;
use env_logger;
use log::{info, LevelFilter};
use request_dispatch::run_dispatcher;
use std::{process::exit, thread::spawn};
use worldview::Worldview;

mod backup;
mod elevator;
mod network;
mod request_dispatch;
mod requests;
mod timer;
mod worldview;

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

    let elevio_driver =
        elevio::elev::Elevator::init(&format!("localhost:{}", args.port), 4).unwrap();

    let name = args.name.unwrap_or(petname::petname(1, "").unwrap());

    // Load state from backup if available
    let inital_worldview = match load_state_from_file("backupd.json") {
        Ok(mut states) => {
            info!("Loaded backup.");
            states.name = name;
            states
        }
        Err(_) => {
            info!("No backup found.");
            Worldview::new(name)
        }
    };

    let (command_channel_tx, command_channel_rx) = cbc::unbounded();
    let (elevator_event_tx, elevator_event_rx) = cbc::unbounded();

    {
        let elevio_driver = elevio_driver.clone();
        spawn(move || controller_loop(&elevio_driver, command_channel_rx, elevator_event_tx));
    }

    run_dispatcher(
        inital_worldview,
        &elevio_driver,
        command_channel_tx,
        elevator_event_rx,
    );

    exit(1);
}
