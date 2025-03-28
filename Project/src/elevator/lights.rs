use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};

pub fn set_cab_lights(elevio_driver: &Elevator, requests: &Vec<bool>) {
    for (floor, &on) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(floor, CAB, on);
        elevio_driver.call_button_light(floor, HALL_UP, on);
        elevio_driver.call_button_light(floor, HALL_DOWN, on);
    }
}

pub fn set_hall_lights(elevio_driver: &Elevator, requests: &Vec<(bool, bool)>) {
    for (floor, &(up_on, down_on)) in requests.iter().enumerate() {
        let floor = floor as u8;

        elevio_driver.call_button_light(floor, HALL_UP, up_on);
        elevio_driver.call_button_light(floor, HALL_DOWN, down_on);
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
