mod inputs;
mod states;
mod timer;

use std::time::Duration;

use crossbeam_channel as cbc;

use driver_rust::elevio::elev::{self as e, DIRN_STOP};
use states::States;

fn main() -> std::io::Result<()> {
    let elev_num_floors = 4;
    let elevio_elevator = e::Elevator::init("localhost:15657", elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevio_elevator);

    let mut dirn = e::DIRN_DOWN;
    if elevio_elevator.floor_sensor().is_none() {
        elevio_elevator.motor_direction(dirn);
    }

    let (timer_channel_tx, timer_channel_rx) = cbc::unbounded::<()>();
    let rx_channels = inputs::get_input_channels(&elevio_elevator);
    let mut elevator = states::Elevator::init();

    loop {
        cbc::select! {
            recv(rx_channels.call_button_rx) -> a => {
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
                        elevator.state = States::Moving;

                        // TODO: Set motor direction
                    },
                    _ => {

                    },
                }                
            },
            recv(rx_channels.floor_sensor_rx) -> a => {
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);
     
                match elevator.state {
                    States::Moving => {
                        if elevator.should_stop() {
                            elevator.state = States::DoorOpen;
                            
                            timer::start_timer(Duration::from_secs(3), &timer_channel_tx);

                            elevio_elevator.motor_direction(DIRN_STOP);
                        }
                    },
                    _ => {

                    },
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

            }
        }
    }
}