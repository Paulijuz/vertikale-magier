use crossbeam_channel as cbc;
use driver_rust::elevio::{self, elev::DIRN_STOP};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::inputs::{
    create_floor_sensor_channel, create_obstruction_channel, create_stop_button_channel,
};
use crate::{elevator::lights::set_state_lights, requests::requests::Requests, timer::Timer};

const DOOR_OPEN_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
    OutOfOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevatorState {
    pub direction: Direction,
    pub behaviour: Behaviour,
    pub obstruction: bool,
    pub floor: usize, // Floor is usize as it's primaraly used for indexing.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElevatorEvent {
    StateUpdated(ElevatorState),
    FloorCleared((usize, Direction)),
}

/// Returns the next direction and behaviour the elevator should move/be in based on
/// current floor, direciton and requests.
fn next_state(floor: usize, direction: Direction, requests: &Requests) -> (Direction, Behaviour) {
    match direction {
        Direction::Up => {
            if requests.up_at_floor(floor) {
                (Direction::Up, Behaviour::DoorOpen)
            } else if requests.any_above_floor(floor) {
                (Direction::Up, Behaviour::Moving)
            } else if requests.any_at_floor(floor) {
                (Direction::Down, Behaviour::DoorOpen)
            } else if requests.any_below_floor(floor) {
                (Direction::Down, Behaviour::Moving)
            } else {
                (Direction::Stopped, Behaviour::Idle)
            }
        }
        Direction::Down => {
            if requests.any_below_floor(floor) {
                (Direction::Down, Behaviour::Moving)
            } else if requests.down_at_floor(floor) {
                (Direction::Down, Behaviour::DoorOpen)
            } else if requests.any_at_floor(floor) {
                (Direction::Up, Behaviour::DoorOpen)
            } else if requests.any_above_floor(floor) {
                (Direction::Up, Behaviour::Moving)
            } else {
                (Direction::Stopped, Behaviour::Idle)
            }
        }
        Direction::Stopped => {
            if requests.any_at_floor(floor) {
                (Direction::Stopped, Behaviour::DoorOpen)
            } else if requests.any_above_floor(floor) {
                (Direction::Up, Behaviour::Moving)
            } else if requests.any_below_floor(floor) {
                (Direction::Down, Behaviour::Moving)
            } else {
                (Direction::Stopped, Behaviour::Idle)
            }
        }
    }
}

/// Returns wheter or not the elevator should stop based on the current
/// floor, direciton and requests.
fn should_stop(floor: usize, direction: Direction, requests: &Requests) -> bool {
    match direction {
        Direction::Down => requests.down_at_floor(floor) || !requests.any_below_floor(floor),
        Direction::Up => requests.up_at_floor(floor) || !requests.any_above_floor(floor),
        Direction::Stopped => true,
    }
}

fn should_instantly_clear(floor: usize, direction: Direction, requests: &Requests) -> bool {
    match direction {
        Direction::Down => requests.down_at_floor(floor),
        Direction::Up => requests.up_at_floor(floor),
        Direction::Stopped => requests.any_at_floor(floor),
    }
}

/// Starts the motor in the given direction.
///
/// **Note:** Trying to start the motor in the direction `Stopped` will return an error.
fn start_motor(elevio_driver: &elevio::elev::Elevator, direction: Direction) -> Result<(), ()> {
    match direction {
        Direction::Up => elevio_driver.motor_direction(elevio::elev::DIRN_UP),
        Direction::Down => elevio_driver.motor_direction(elevio::elev::DIRN_DOWN),
        _ => return Err(()),
    }

    Ok(())
}

/// Tries opening the elevator door. Will return an error if the elevator is
/// not stopped at a floor.
fn open_door(elevio_driver: &elevio::elev::Elevator) -> Result<(), ()> {
    // There is no way to check that the motor is stopped with elevio,
    // so we'll just set the motor to be stopped to be sure.
    elevio_driver.motor_direction(elevio::elev::DIRN_STOP);

    if elevio_driver.floor_sensor().is_none() {
        warn!("Tried opening door while not stopped at a floor.");
        return Err(());
    }

    elevio_driver.door_light(true);
    debug!("Door open.");

    Ok(())
}

/// Tries closing the elevator door. Will return an error if the door is obstructed.
fn close_door(elevio_driver: &elevio::elev::Elevator) -> Result<(), ()> {
    if elevio_driver.obstruction() {
        warn!("Tried closing door while obstructed.");
        return Err(());
    }

    elevio_driver.door_light(false);
    debug!("Door closed.");

    Ok(())
}

//Initialize the elevator position to the bottom floor
fn initialize_elevator_position(elevio_driver: &elevio::elev::Elevator) {
    debug!("Initializing elevator position.");
    elevio_driver.motor_direction(elevio::elev::DIRN_DOWN);
    while elevio_driver.floor_sensor().is_none() {
        std::thread::sleep(Duration::from_millis(50));
    }
    elevio_driver.motor_direction(elevio::elev::DIRN_STOP); 
    debug!("Elevator initialized at floor {}," , elevio_driver.floor_sensor().unwrap());
}
    

/// The main loop for the elevator FSM. It is an event based loop that listen for events from
/// the elevio driver and the `command_channel`. TODO: Further explain command channel.
///
/// **Note:** This function blocks execution.
pub fn controller_loop(
    elevio_driver: &elevio::elev::Elevator,
    elevator_command_rx: cbc::Receiver<Requests>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {

    initialize_elevator_position(elevio_driver);

    let floor_sensor_channel = create_floor_sensor_channel(elevio_driver);
    let obstruction_channel = create_obstruction_channel(elevio_driver);
    let stop_button_channel = create_stop_button_channel(elevio_driver);

    let mut door_timer = Timer::new(DOOR_OPEN_DURATION);

    let mut requests = Requests::new(elevio_driver.num_floors as usize);
    let mut state = ElevatorState {
        behaviour: Behaviour::Idle,
        direction: Direction::Stopped,
        obstruction: true, // Assume worst until we hear otherwise
        floor: elevio_driver.floor_sensor().unwrap_or(0) as usize, //Using detected floor
    };
    let mut previous_state: Option<ElevatorState> = None;

    loop {
        // Only send the state if it has changed.
        if Some(state) != previous_state {
            previous_state = Some(state);
            elevator_event_tx
                .send(ElevatorEvent::StateUpdated(state))
                .unwrap();
            set_state_lights(elevio_driver, state);
        }

        cbc::select! {
            recv(elevator_command_rx) -> command => {
                requests = command.unwrap();
                debug!("Elevator controller recieved new requests: {:?}", requests);

                match state.behaviour {
                    Behaviour::DoorOpen => {
                        if should_instantly_clear(state.floor, state.direction, &requests) {
                            elevator_event_tx.send(ElevatorEvent::FloorCleared((state.floor, state.direction))).unwrap();
                            door_timer.restart();
                        }
                    },
                    Behaviour::Idle => {
                        (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);

                        match state.behaviour {
                            Behaviour::DoorOpen => {
                                open_door(elevio_driver);
                                elevator_event_tx.send(ElevatorEvent::FloorCleared((state.floor, state.direction))).unwrap();
                                door_timer.start();
                            },
                            Behaviour::Moving => {
                                start_motor(elevio_driver, state.direction);
                            },
                            _ => {},
                        }
                    },
                    _ => {},
                }
            },
            recv(floor_sensor_channel) -> floor => {
                state.floor = floor.unwrap() as usize;
                debug!("Detected floor: {}", state.floor);

                if state.behaviour != Behaviour::Moving {
                    continue;
                }

                if should_stop(state.floor, state.direction, &requests) {
                    elevio_driver.motor_direction(DIRN_STOP);

                    (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);

                    if requests.any_at_floor(state.floor) {
                        state.behaviour = Behaviour::DoorOpen;
                        open_door(elevio_driver);
                        elevator_event_tx.send(ElevatorEvent::FloorCleared((state.floor, state.direction))).unwrap();
                        door_timer.start();
                    } else {
                        state.behaviour = Behaviour::Idle;
                    }
                }
            },
            recv(stop_button_channel) -> stop_button => {
                let stop_button = stop_button.unwrap();
                debug!("Detected stop-button: {:}", stop_button);

                if !stop_button {
                    continue;
                }

                elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
                state.behaviour = Behaviour::OutOfOrder;
            },
            recv(obstruction_channel) -> obstruction => {
                state.obstruction = obstruction.unwrap();
                debug!("Detected obstruction: {:}", state.obstruction);

                if state.obstruction {
                    door_timer.stop();
                } else {
                    door_timer.start();
                }
            },
            recv(door_timer.timeout_channel()) -> _ => {
                if state.obstruction {
                    debug!("Door obstructed!");
                    continue;
                }

                close_door(elevio_driver);

                (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);

                match state.behaviour {
                    Behaviour::DoorOpen => {
                        open_door(elevio_driver);
                        elevator_event_tx.send(ElevatorEvent::FloorCleared((state.floor, state.direction))).unwrap();
                        door_timer.start();
                    },
                    Behaviour::Moving => {
                        start_motor(elevio_driver, state.direction);
                    },
                    _ => {},
                }
            },
        }
    }
}
