use crossbeam_channel::{self as cbc, Receiver, Sender};
use driver_rust::elevio::poll::CallButton;
use driver_rust::elevio::{elev::Elevator, poll};
use std::thread::spawn;
use std::time::Duration;

const POLL_PERIOD: Duration = Duration::from_millis(25);

/// Utility function for wrapping elevio "poll functions" with channels.
/// Optionally an "inital function" can be passed which will be called
/// once when the channel is first created.
fn create_poll_channel<T: Send + 'static>(
    elevator: &Elevator,
    poll_function: fn(Elevator, cbc::Sender<T>, Duration),
    inital_function: Option<fn(&Elevator) -> T>,
) -> cbc::Receiver<T> {
    let elevator = elevator.to_owned();
    let (channel_tx, channel_rx) = cbc::unbounded::<T>();

    if let Some(inital_function) = inital_function {
        channel_tx.send(inital_function(&elevator)).unwrap();
    }

    spawn(move || poll_function(elevator, channel_tx, POLL_PERIOD));

    channel_rx
}

pub fn create_call_button_channel(elevio_driver: &Elevator) -> Receiver<CallButton> {
    create_poll_channel(elevio_driver, poll::call_buttons, None)
}

pub fn create_floor_sensor_channel(elevio_driver: &Elevator) -> Receiver<u8> {
    create_poll_channel(elevio_driver, poll::floor_sensor, None)
}

pub fn create_obstruction_channel(elevio_driver: &Elevator) -> Receiver<bool> {
    create_poll_channel(
        elevio_driver,
        poll::obstruction,
        Some(Elevator::obstruction),
    )
}

pub fn create_stop_button_channel(elevio_driver: &Elevator) -> Receiver<bool> {
    create_poll_channel(
        elevio_driver,
        poll::stop_button,
        Some(Elevator::stop_button),
    )
}
