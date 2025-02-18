mod inputs;
mod states;
mod timer;

use std::time::Duration;

use crossbeam_channel::{self as cbc, Sender};

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP, DIRN_UP}; // TODO: Ikke importer som e
use states::{Direction, Elevator, States};

fn choose_direction(elevator: &Elevator) -> (Direction, States) {
    // TODO: Flytte ut fra main
    match elevator.direction {
        Direction::Up => {
            return if elevator.orders_above() {
                (Direction::Up, States::Moving)
            } else if elevator.orders_here() {
                (Direction::Down, States::DoorOpen)
            } else if elevator.orders_below() {
                (Direction::Down, States::Moving)
            } else {
                (Direction::Stopped, States::Idle)
            }
        }
        Direction::Down => {
            return if elevator.orders_below() {
                (Direction::Down, States::Moving)
            } else if elevator.orders_here() {
                (Direction::Up, States::DoorOpen)
            } else if elevator.orders_above() {
                (Direction::Up, States::Moving)
            } else {
                (Direction::Stopped, States::Idle)
            }
        }
        Direction::Stopped => {
            return if elevator.orders_here() {
                (Direction::Stopped, States::DoorOpen)
            } else if elevator.orders_above() {
                (Direction::Up, States::Moving)
            } else if elevator.orders_below() {
                (Direction::Down, States::Moving)
            } else {
                (Direction::Stopped, States::Idle)
            }
        }
    }
}

fn start_moving(
    elevator: &mut Elevator,
    elevio_elevator: &e::Elevator,
    timer_channel_tx: &Sender<()>,
) {
    let (direction, state) = choose_direction(elevator);

    elevator.state = state;

    if state == States::DoorOpen {
        println!("Stopping in move!");
        elevator.clear_orders_here();
        timer::start_timer(Duration::from_secs(3), &timer_channel_tx);
    }

    match direction {
        Direction::Up => {
            elevio_elevator.motor_direction(DIRN_UP);
            elevator.direction = Direction::Up;
        }
        Direction::Down => {
            elevio_elevator.motor_direction(DIRN_DOWN);
            elevator.direction = Direction::Down;
        }
        Direction::Stopped => {
            elevio_elevator.motor_direction(DIRN_STOP);
        }
    }
}

fn main() -> std::io::Result<()> {
    let elev_num_floors = 4;
    let elevio_elevator = e::Elevator::init("localhost:15657", elev_num_floors)?; // TODO: Slå sammen elevio_elevator og elevator kanskje?
    println!("Elevator started:\n{:#?}", elevio_elevator);

    let (timer_channel_tx, timer_channel_rx) = cbc::unbounded::<()>();
    let rx_channels = inputs::get_input_channels(&elevio_elevator);
    let mut elevator = states::Elevator::init();

    loop {
        cbc::select! { // TODO: Denne logikken bør flyttes til en egen fil
            recv(rx_channels.call_button_rx) -> a => { // TODO: Gi alle variablene "a" et bedre navn kanskje
                let call_button = a.unwrap();
                println!("{:#?}", call_button);
                elevio_elevator.call_button_light(call_button.floor, call_button.call, true);

                match call_button.call {
                    0 => {
                        elevator.orders[call_button.floor as usize].outside_call_up = true;
                    }
                    1 => {
                        elevator.orders[call_button.floor as usize].outside_call_down = true;
                    }
                    2=> {
                        elevator.orders[call_button.floor as usize].inside_call = true;
                    }
                    _ => {
                        panic!("Fikk ukjent knapp.");
                    }
                }

                match elevator.state {
                    States::Idle => {
                        start_moving(&mut elevator, &elevio_elevator, &timer_channel_tx);
                    },
                    _ => {},
                }
            },
            recv(rx_channels.floor_sensor_rx) -> a => {
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);

                elevator.floor = floor;

                match elevator.state {
                    States::Moving => {
                        if elevator.should_stop() {
                            println!("Stopping!");
                            elevator.state = States::DoorOpen;
                            elevator.clear_orders_here();
                            elevio_elevator.motor_direction(DIRN_STOP);

                            timer::start_timer(Duration::from_secs(1), &timer_channel_tx);
                        }
                    },
                    _ => {},
                }
            },
            recv(rx_channels.stop_button_rx) -> a => {
                let stop = a.unwrap();
                println!("Stop button: {:#?}", stop);

                match elevator.state {
                    _ => {
                        elevator.state = States::OutOfOrder;
                    },
                }
            },
            recv(rx_channels.obstruction_rx) -> a => {
                let obstr = a.unwrap();
                println!("Obstruction: {:#?}", obstr);

                match elevator.state {

                    States::DoorOpen => {
                        // Ikke lukk dør
                    },
                    _ => {

                    },
                }
            },
            recv(timer_channel_rx) -> _ => {
                println!("Door close!");
                start_moving(&mut elevator, &elevio_elevator, &timer_channel_tx);
            }
        }
    }
}
