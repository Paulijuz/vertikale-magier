mod elevator_controller;
mod hall_request_assigner;
mod inputs;
mod light_sync;
mod network;
mod request_dispatch;
mod timer;
mod backup;

use backup::{load_state_from_file, save_state_to_file};
use crossbeam_channel as cbc;
use driver_rust::elevio;
use elevator_controller::{controller_loop, Direction, State};
use env_logger;
use log::{error, info, LevelFilter};
use request_dispatch::{start_master_server, start_slave_client};
use std::{env, thread::spawn};
use crate::request_dispatch::{AllElevatorStates, SingleElevatorState};


fn main() {

    env_logger::Builder::new()
        .filter_level(LevelFilter::Trace)
        .init();

    // if env::args().any(|arg| arg == "master") {
    //     start_master_server();
    //     return;
    // }

    // if env::args().any(|arg| arg == "slave") {
    //     let elevio_driver: elevio::elev::Elevator =
    //         elevio::elev::Elevator::init("localhost:15657", 4).unwrap();

    //     let (command_channel_tx, command_channel_rx) = cbc::unbounded();
    //     let (elevator_event_tx, elevator_event_rx) = cbc::unbounded();

    //     {
    //         let elevio_driver = elevio_driver.clone();
    //         spawn(move || controller_loop(&elevio_driver, command_channel_rx, elevator_event_tx));
    //     }

    //     start_slave_client(&elevio_driver, command_channel_tx, elevator_event_rx);
    //     return;
    // }

    // error!("Programmet må startes som enten master eller slave. Kjør 'cargo run master' for master eller 'cargo run slave' for slave.");
    // // Tester backup-lagring og -lasting   
    let test_file = "backup.json";

    info!("Starter test av backup-systemet...");

    // Opprett en testtilstand
    let mut test_state = AllElevatorStates::new();
    let test_elevator = SingleElevatorState {
        name: "TestHeis".to_string(),
        direction: Direction::Up,
        state: State::Idle,
        floor: 3,
        cab_requests: [false, false, true, false],
    };
    test_state.elevators.insert("TestHeis".to_string(), test_elevator.clone());

    // Lagre til backup-fil
    if let Err(e) = save_state_to_file(&test_state, test_file) {
        error!("Feil ved lagring: {}", e);
        return;
    }
    info!("Lagret testdata i `{}`", test_file);

    // Les tilbake fra fil
    match load_state_from_file(test_file) {
        Ok(loaded_state) => {
            info!("Lastet inn data fra `{}`", test_file);

            // Sjekk at verdiene samsvarer
            if loaded_state.elevators.get("TestHeis") == Some(&test_elevator) {
                info!("Test vellykket: Lagret og lastet tilstand er like!");
            } else {
                error!(" Test feilet: Verdiene er ikke like.");
            }
        }
        Err(e) => error!("Feil ved lasting: {}", e),
    }
}
