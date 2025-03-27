use clap::Parser;
use crossbeam_channel as cbc;
use driver_rust::elevio;
use env_logger;
use std::{process, thread};

mod backup;
mod elevator;
mod hall_request_assigner;
mod message;
mod network;
mod request_dispatch;
mod requests;
mod timer;

#[derive(Debug, Parser)]
/// Group 52's amazing distributed elevator control system.
struct Args {
    /// Name to use as identifier for this elevator. If not specified a random name will be chosen.
    #[arg(long, short)]
    name: Option<String>,

    /// Port of the elevator server to connect to.
    #[arg(long, short, default_value_t = 15657)]
    port: u16,

    /// Number of floors the elevator has.
    #[arg(long, short = 'f', default_value_t = 4)]
    num_floors: usize,
}

fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let args = Args::parse();

    let elevio_driver =
        elevio::elev::Elevator::init(&format!("localhost:{}", args.port), args.num_floors as u8)
            .unwrap();

    // Load state from backup if available
    // let inital_worldview = match backup::load_state_from_file("backup.json") {
    //     Ok(worldview) => {
    //         info!("Loaded backup.");
    //         worldview
    //     }
    //     Err(_) => {
    //         info!("No backup found.");
    //         let name = args.name.unwrap_or(petname::petname(1, "").unwrap());
    //         worldview::Worldview::new(name, args.num_floors)
    //     }
    // };

    let name = args.name.unwrap_or(petname::petname(1, "").unwrap());

    let node = network::Node::new(name.clone());

    let (elevator_command_tx, elevator_command_rx) = cbc::unbounded();
    let (elevator_event_tx, elevator_event_rx) = cbc::unbounded();

    {
        let elevio_driver = elevio_driver.clone();
        thread::spawn(move || {
            elevator::controller::controller_loop(
                &elevio_driver,
                elevator_command_rx,
                elevator_event_tx,
            )
        });
    }

    request_dispatch::run_dispatcher(
        name,
        node,
        &elevio_driver,
        elevator_command_tx,
        elevator_event_rx,
    );

    process::exit(1);
}
