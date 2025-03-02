use crossbeam_channel as cbc;
use driver_rust::elevio::{elev::Elevator, poll};
use std::thread::spawn;
use std::time::Duration;

const POLL_PERIOD: Duration = Duration::from_millis(25);

pub struct InputChannels {
    pub call_button_rx: cbc::Receiver<poll::CallButton>,
    pub floor_sensor_rx: cbc::Receiver<u8>,
    pub stop_button_rx: cbc::Receiver<bool>,
    pub obstruction_rx: cbc::Receiver<bool>,
}

/// Utility function for wrapping elevio "poll functions" with channels.
/// Optionally an "inital function" can be passed which will be called
/// once when the channel is first created.
fn create_poll_channel<T: Send + 'static>(
    elevator: &Elevator,
    poll_function: fn(Elevator, cbc::Sender<T>, Duration),
    inital_function: Option<fn(&Elevator) -> T>,
) -> cbc::Receiver<T> {
    let elevator = elevator.to_owned();
    let (tx, rx) = cbc::unbounded::<T>();

    if let Some(inital_function) = inital_function {
        tx.send(inital_function(&elevator)).unwrap();
    }

    spawn(move || poll_function(elevator, tx, POLL_PERIOD));

    rx
}

impl InputChannels {
    pub fn new(elevator: &Elevator) -> Self {
        Self {
            call_button_rx: create_poll_channel(elevator, poll::call_buttons, None),
            floor_sensor_rx: create_poll_channel(elevator, poll::floor_sensor, None),
            obstruction_rx: create_poll_channel(
                elevator,
                poll::obstruction,
                Some(Elevator::obstruction),
            ),
            stop_button_rx: create_poll_channel(
                elevator,
                poll::stop_button,
                Some(Elevator::stop_button),
            ),
        }
    }
}
