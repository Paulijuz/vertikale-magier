use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

use super::requests::Requests;
use crate::worldview::{HallRequest, HallRequestState}; // TODO: Remove this

pub fn set_cab_lights(elevio_driver: &Elevator, requests: &Requests) {
    for (floor, ((&up, &down), &cab)) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(floor, CAB, cab);
        elevio_driver.call_button_light(floor, HALL_UP, up);
        elevio_driver.call_button_light(floor, HALL_DOWN, down);
    }
}

pub fn set_hall_lights(elevio_driver: &Elevator, requests: &Vec<HallRequest>) {
    for (floor, hall_request) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(
            floor,
            HALL_UP,
            matches!(hall_request.up, HallRequestState::Assigned(_)),
        );
        elevio_driver.call_button_light(
            floor,
            HALL_DOWN,
            matches!(hall_request.down, HallRequestState::Assigned(_)),
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
