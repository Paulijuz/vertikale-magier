use crate::elevator_controller::ElevatorRequests;
use driver_rust::elevio::elev::Elevator;

pub fn sync_call_lights(elevator: &Elevator, requests: &ElevatorRequests) {
    for (floor, request) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, 0, request.hall_up);
        elevator.call_button_light(floor, 1, request.hall_down);
        elevator.call_button_light(floor, 2, request.cab);
    }
}
