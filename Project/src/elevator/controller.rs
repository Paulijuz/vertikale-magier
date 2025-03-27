use crossbeam_channel as cbc;
use driver_rust::elevio;
use log::{error, debug, warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{
    inputs::{create_call_button_channel, create_floor_sensor_channel, create_obstruction_channel, create_stop_button_channel},
    requests::{Direction, RequestType, Requests},
};
use crate::{elevator::lights::{set_cab_lights}, timer::Timer};

const DOOR_OPEN_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
    OutOfOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevatorState {
    pub direction: Option<Direction>,
    pub behaviour: Behaviour,
    pub obstruction: bool,
    pub floor: usize, // Floor is usize as it's primaraly used for indexing.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElevatorEvent {
    StateUpdated(ElevatorState),
    RequestCleared(usize, RequestType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElevatorCommand {
    AddRequest(usize, RequestType),
    ClearRequest(usize, RequestType),
}

/// Returns the next direction and behaviour the elevator should move/be in based on
/// current floor, direciton and requests.
fn next_state(floor: usize, direction: Option<Direction>, requests: &Requests) -> (Option<Direction>, Behaviour) {
    match direction {
        Some(Direction::Up) => {
            if requests.up_at_floor(floor) {
                error!("a case 1");
                (Some(Direction::Up), Behaviour::DoorOpen)
            } else if requests.any_above_floor(floor) {
                error!("a case 2");
                (Some(Direction::Up), Behaviour::Moving)
            } else if requests.down_at_floor(floor) {
                error!("a case 3");
                (Some(Direction::Down), Behaviour::DoorOpen)
            } else if requests.any_below_floor(floor) {
                error!("a case 4");
                (Some(Direction::Down), Behaviour::Moving)
            } else {
                (None, Behaviour::Idle)
            }
        }
        _ => {
            if requests.down_at_floor(floor) {
                error!("b case 1");
                (Some(Direction::Down), Behaviour::DoorOpen)
            } else if requests.any_below_floor(floor) {
                error!("b case 2");
                (Some(Direction::Down), Behaviour::Moving)
            } else if requests.up_at_floor(floor) {
                error!("b case 3");
                (Some(Direction::Up), Behaviour::DoorOpen)
            } else if requests.any_above_floor(floor) {
                error!("b case 4");
                (Some(Direction::Up), Behaviour::Moving)
            } else {
                (None, Behaviour::Idle)
            }
        }
    }
}

/// Returns wheter or not the elevator should stop based on the current
/// floor, direciton and requests.
fn should_stop(floor: usize, direction: Option<Direction>, requests: &Requests) -> bool {
    match direction {
        Some(Direction::Down) => requests.down_at_floor(floor) || !requests.any_below_floor(floor),
        Some(Direction::Up) => requests.up_at_floor(floor) || !requests.any_above_floor(floor),
        None => true,
    }
}

fn should_instantly_clear(floor: usize, direction: Option<Direction>, requests: &Requests) -> bool {
    match direction {
        Some(Direction::Down) => requests.down_at_floor(floor),
        Some(Direction::Up) => requests.up_at_floor(floor),
        None => requests.any_at_floor(floor),
    }
}

/// Starts the motor in the given direction.
///
/// **Note:** Trying to start the motor in the direction `Stopped` will return an error.
fn start_motor(elevio_driver: &elevio::elev::Elevator, direction: Option<Direction>) -> Result<(), ()> {
    match direction {
        Some(Direction::Up) => elevio_driver.motor_direction(elevio::elev::DIRN_UP),
        Some(Direction::Down) => elevio_driver.motor_direction(elevio::elev::DIRN_DOWN),
        None => return Err(()),
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
fn initialize_elevator_position(elevio_driver: &elevio::elev::Elevator) -> usize {
    debug!("Initializing elevator position.");
    elevio_driver.motor_direction(elevio::elev::DIRN_DOWN);

    loop {
        if let Some(floor) = elevio_driver.floor_sensor() {
            debug!("Elevator initialized at floor {floor}.");
            elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
            return floor as usize;
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn clear_floor(
    requests: &mut Requests,
    elevator_event_tx: &cbc::Sender<ElevatorEvent>,
    floor: usize,
    direction: Option<Direction>,
) {
    debug!("Clearing: {direction:?}");

    requests.clear(floor, RequestType::Cab);    
    elevator_event_tx.send(ElevatorEvent::RequestCleared(floor, RequestType::Cab)).unwrap();

    if let Some(direction) = direction {
        requests.clear(floor, RequestType::Hall(direction));
        elevator_event_tx.send(ElevatorEvent::RequestCleared(floor, RequestType::Hall(direction))).unwrap();
    }
}

/// The main loop for the elevator FSM. It is an event based loop that listen for events from
/// the elevio driver and the `command_channel`. TODO: Further explain command channel.
///
/// **Note:** This function blocks execution.
pub fn controller_loop(
    elevio_driver: &elevio::elev::Elevator,
    elevator_command_rx: cbc::Receiver<ElevatorCommand>,
    elevator_command_tx: cbc::Sender<ElevatorCommand>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {
    let inital_floor = initialize_elevator_position(elevio_driver);

    let floor_sensor_channel = create_floor_sensor_channel(elevio_driver);
    let obstruction_channel = create_obstruction_channel(elevio_driver);
    let stop_button_channel = create_stop_button_channel(elevio_driver);
    let call_button_channel = create_call_button_channel(elevio_driver);

    let mut door_timer = Timer::new(DOOR_OPEN_DURATION);

    let mut requests = Requests::new(elevio_driver.num_floors as usize);
    let mut state = ElevatorState {
        behaviour: Behaviour::Idle,
        direction: None,
        obstruction: elevio_driver.obstruction(),
        floor: inital_floor,
    };
    let mut previous_state: Option<ElevatorState> = None;

    loop {
        set_cab_lights(elevio_driver, &requests);

        // Only send the state if it has changed.
        if Some(state) != previous_state {
            debug!("{:?}", state);
            previous_state = Some(state);
            elevator_event_tx
                .send(ElevatorEvent::StateUpdated(state))
                .unwrap();
        }

        cbc::select! {
            recv(elevator_command_rx) -> command => {
                let command = command.unwrap();

                match command {
                    ElevatorCommand::AddRequest(floor, request_type) => requests.add(floor, request_type),
                    ElevatorCommand::ClearRequest(floor, request_type) => requests.clear(floor, request_type),
                }

                match state.behaviour {
                    Behaviour::DoorOpen => {
                        if should_instantly_clear(state.floor, state.direction, &requests) {
                            clear_floor(&mut requests, &elevator_event_tx, state.floor, state.direction);
                            door_timer.restart();
                        }
                    },
                    Behaviour::Idle => {
                        (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);
                        debug!("Changed direction to: {:?}", state.direction);

                        match state.behaviour {
                            Behaviour::DoorOpen => {
                                open_door(elevio_driver);
                                clear_floor(&mut requests, &elevator_event_tx, state.floor, state.direction);
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

                elevio_driver.floor_indicator(state.floor as u8);

                if state.behaviour != Behaviour::Moving {
                    continue;
                }

                if should_stop(state.floor, state.direction, &requests) {
                    elevio_driver.motor_direction(elevio::elev::DIRN_STOP);

                    (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);
                    debug!("Changed direction to: {:?}", state.direction);

                    if requests.any_at_floor(state.floor) {
                        open_door(elevio_driver);
                        clear_floor(&mut requests, &elevator_event_tx, state.floor, state.direction);
                        door_timer.start();
                        state.behaviour = Behaviour::DoorOpen;
                        
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

                if state.behaviour == Behaviour::DoorOpen {
                    if state.obstruction {
                        door_timer.stop();
                    } else {
                        door_timer.start();
                    }
                }
            },
            recv(door_timer.timeout_channel()) -> _ => {
                if state.obstruction {
                    debug!("Door obstructed!");
                    continue;
                }

                close_door(elevio_driver);

                (state.direction, state.behaviour) = next_state(state.floor, state.direction, &requests);
                debug!("Changed direction to: {:?}", state.direction);

                match state.behaviour {
                    Behaviour::DoorOpen => {
                        open_door(elevio_driver);
                        clear_floor(&mut requests, &elevator_event_tx, state.floor, state.direction);
                        door_timer.start();
                    },
                    Behaviour::Moving => {
                        start_motor(elevio_driver, state.direction);
                    },
                    _ => {},
                }
            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;

                //Add order at floor
                match call_button.call {
                    HALL_UP => elevator_command_tx.send(ElevatorCommand::AddRequest(floor, RequestType::Hall(Direction::Up))).unwrap(),
                    HALL_DOWN => elevator_command_tx.send(ElevatorCommand::AddRequest(floor, RequestType::Hall(Direction::Down))).unwrap(),
                    CAB => elevator_command_tx.send(ElevatorCommand::AddRequest(floor, RequestType::Cab)).unwrap(),
                    _ => {},
                }
            },
        }
    }
}
