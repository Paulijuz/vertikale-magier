use crossbeam_channel as cbc;
use std::thread::spawn;
use std::time::Duration;
use driver_rust::elevio;

pub struct RxChannels {
    pub call_button_rx: cbc::Receiver<elevio::poll::CallButton>,
    pub floor_sensor_rx: cbc::Receiver<u8>,
    pub stop_button_rx: cbc::Receiver<bool>,
    pub obstruction_rx: cbc::Receiver<bool>,
}

pub fn get_input_channels(elevator: &elevio::elev::Elevator) -> RxChannels {
    let poll_period = Duration::from_millis(25);

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    let (floor_sensor_tx, floor_sensor_rx) = cbc::unbounded::<u8>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::floor_sensor(elevator, floor_sensor_tx, poll_period));
    }

    let (stop_button_tx, stop_button_rx) = cbc::unbounded::<bool>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::stop_button(elevator, stop_button_tx, poll_period));
    }

    let (obstruction_tx, obstruction_rx) = cbc::unbounded::<bool>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::obstruction(elevator, obstruction_tx, poll_period));
    }

    return RxChannels {
        call_button_rx,
        floor_sensor_rx,
        obstruction_rx,
        stop_button_rx,
    };
}
