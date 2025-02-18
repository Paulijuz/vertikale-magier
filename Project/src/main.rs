use crossbeam_channel as cbc;
use driver_rust::elevio;
use elevator_controller::controller_loop;
use order_dispatch::dispatch_loop;
use std::thread;

mod elevator_controller;
mod inputs;
mod light_sync;
mod order_dispatch;
mod timer;

fn main() {
    let elev_num_floors = 4;
    let elevio_elevator = elevio::elev::Elevator::init("localhost:15657", elev_num_floors).unwrap(); // TODO: Slå sammen med en annen struct på en eller annen måte?
    println!("Elevator started:\n{:#?}", elevio_elevator);

    let (elevator_command_tx, elevator_controller_rx) =
        cbc::unbounded::<elevator_controller::ElevatorOrders>();
    let (elevator_event_tx, elevator_event_rx) =
        cbc::unbounded::<elevator_controller::ElevatorEvent>();

    let elevio_elevator_cloned = elevio_elevator.clone();
    thread::spawn(move || controller_loop(&elevio_elevator_cloned, elevator_controller_rx, elevator_event_tx));

    dispatch_loop(&elevio_elevator, elevator_command_tx, elevator_event_rx);
}

