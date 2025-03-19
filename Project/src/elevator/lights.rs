use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

use crate::{requests::requests::{Requests, NUMBER_OF_FLOORS}, worldview::{HallRequest, HallRequestState}};

pub fn set_cab_lights(elevator: &Elevator, requests: &Requests) {
    for (floor, (_, cab)) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, CAB, cab);
    }
}

pub fn set_hall_lights(elevator: &Elevator, requests: &[HallRequest; NUMBER_OF_FLOORS]) {
    for (floor, hall_request) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, HALL_UP, hall_request.up != HallRequestState::Inactive);
        elevator.call_button_light(floor, HALL_DOWN, hall_request.down != HallRequestState::Inactive);
    }
}