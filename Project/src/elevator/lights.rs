use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

use crate::{
    requests::requests::Requests,
    worldview::{HallRequest, HallRequestState},
};

use super::controller::{Behaviour, ElevatorState};

pub fn set_state_lights(elevio_driver: &Elevator, state: ElevatorState) {
    elevio_driver.floor_indicator(state.floor as u8);
    elevio_driver.door_light(state.behaviour == Behaviour::DoorOpen);
}

pub fn set_cab_lights(elevio_driver: &Elevator, requests: &Requests) {
    for (floor, (_, &cab)) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(floor, CAB, cab);
    }
}

pub fn set_hall_lights(elevio_driver: &Elevator, requests: &Vec<HallRequest>) {
    for (floor, hall_request) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(
            floor,
            HALL_UP,
            hall_request.up != HallRequestState::Inactive,
        );
        elevio_driver.call_button_light(
            floor,
            HALL_DOWN,
            hall_request.down != HallRequestState::Inactive,
        );
    }
}

pub fn clear_all_lights(elevio_driver: &Elevator) {
    for i in 0..elevio_driver.num_floors {
        elevio_driver.call_button_light(i, CAB, false);
        elevio_driver.call_button_light(i, HALL_UP, false);
        elevio_driver.call_button_light(i, HALL_DOWN, false);
    }

    // It is not possible to turn of the floor indicator so we'll just set it to the first floor.
    elevio_driver.floor_indicator(0);
    elevio_driver.stop_button_light(false);
}