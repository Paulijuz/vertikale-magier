use crate::elevator_controller::ElevatorOrders;
use driver_rust::elevio::elev::Elevator;

pub fn sync_call_lights(elevator: &Elevator, orders: &ElevatorOrders) {
    for (floor, order) in orders.iter().enumerate() {
        let floor = floor as u8;

        elevator.call_button_light(floor, 0, order.outside_call_up);
        elevator.call_button_light(floor, 1, order.outside_call_down);
        elevator.call_button_light(floor, 2, order.inside_call);
    }
}
