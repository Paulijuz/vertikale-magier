use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

use crate::requests::requests::Requests;

pub fn set_call_lights(elevator: &Elevator, requests: &Requests) {
    for (floor, ((up, down), cab)) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, HALL_UP, up);
        elevator.call_button_light(floor, HALL_DOWN, down);
        elevator.call_button_light(floor, CAB, cab);
    }
}
