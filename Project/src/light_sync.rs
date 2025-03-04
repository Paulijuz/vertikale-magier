use crate::elevator_controller::ElevatorRequests;
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

pub fn sync_call_lights(elevator: &Elevator, requests: &ElevatorRequests) {
    for (floor, request) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, HALL_UP, request.hall_up);
        elevator.call_button_light(floor, HALL_DOWN, request.hall_down);
        elevator.call_button_light(floor, CAB, request.cab);
    }
}
