use crate::inputs;
use crate::timer;
use crossbeam_channel as cbc;
use driver_rust::elevio;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP};
use log::info;
use serde::{Deserialize, Serialize};
use std::time::{self, Duration};

const DOOR_OPEN_DURATION: time::Duration = time::Duration::from_secs(3);
pub const NUMBER_OF_FLOORS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Idle,
    Moving,
    DoorOpen,
    OutOfOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Stopped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Request {
    pub hall_up: bool,
    pub hall_down: bool,
    pub cab: bool,
}

pub type ElevatorRequests = [Request; NUMBER_OF_FLOORS];

pub struct ElevatorEvent {
    pub direction: Direction,
    pub state: State,
    pub floor: u8,
}

// TODO: Ville kanskje vært bedre om man hadde en "ElevatorController" struct
struct ElevatorState {
    fsm_state: State,
    direction: Direction,
    obstruction: bool,
    last_floor: Option<u8>,
    requests: ElevatorRequests,
}

impl ElevatorState {
    pub fn requests_below(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.requests[..floor as usize]
            .iter()
            .any(|request| request.cab || request.hall_down || request.hall_up)
    }
    pub fn requests_above(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.requests[floor as usize + 1..]
            .iter()
            .any(|request| request.cab || request.hall_down || request.hall_up)
    }
    pub fn requests_here(&self, direction: Option<Direction>) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        let request = self.requests[floor as usize];

        match direction {
            Some(Direction::Up) => request.cab || request.hall_up,
            Some(Direction::Down) => request.cab || request.hall_up,
            _ => request.cab || request.hall_up || request.hall_down,
        }
    }
    pub fn next_direction(&self) -> (Direction, State) {
        // TODO: Flytte ut fra main
        match self.direction {
            Direction::Up => {
                return if self.requests_above() {
                    (Direction::Up, State::Moving)
                } else if self.requests_here(None) {
                    (Direction::Down, State::DoorOpen)
                } else if self.requests_below() {
                    (Direction::Down, State::Moving)
                } else {
                    (Direction::Stopped, State::Idle)
                }
            }
            Direction::Down => {
                return if self.requests_below() {
                    (Direction::Down, State::Moving)
                } else if self.requests_here(None) {
                    (Direction::Up, State::DoorOpen)
                } else if self.requests_above() {
                    (Direction::Up, State::Moving)
                } else {
                    (Direction::Stopped, State::Idle)
                }
            }
            Direction::Stopped => {
                return if self.requests_here(None) {
                    (Direction::Stopped, State::DoorOpen)
                } else if self.requests_above() {
                    (Direction::Up, State::Moving)
                } else if self.requests_below() {
                    (Direction::Down, State::Moving)
                } else {
                    (Direction::Stopped, State::Idle)
                }
            }
        }
    }
    pub fn should_stop(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };
        let floor = floor as usize;

        match self.direction {
            Direction::Down => {
                return self.requests[floor].hall_down
                    || self.requests[floor].cab
                    || !self.requests_below()
            }
            Direction::Up => {
                return self.requests[floor].hall_up
                    || self.requests[floor].cab
                    || !self.requests_above()
            }
            _ => return true,
        }
    }
}

// TODO: Denne funksjonen gjør fremdeles mye rart
fn start_moving(
    elevator_state: &mut ElevatorState,
    elevio_elevator: &elevio::elev::Elevator,
    door_timer: &mut timer::Timer,
) {
    let (direction, state) = elevator_state.next_direction();

    info!("{direction:?} {state:?}");

    elevator_state.fsm_state = state;

    if state == State::DoorOpen {
        info!("Stopping in move!");
        elevio_elevator.motor_direction(DIRN_STOP);
        elevator_state.direction = Direction::Stopped;
        door_timer.start(Duration::from_secs(3));
        return;
    }

    match direction {
        Direction::Up => {
            elevio_elevator.motor_direction(DIRN_UP);
            elevator_state.direction = Direction::Up;
        }
        Direction::Down => {
            elevio_elevator.motor_direction(DIRN_DOWN);
            elevator_state.direction = Direction::Down;
        }
        Direction::Stopped => {
            elevio_elevator.motor_direction(DIRN_STOP);
            elevator_state.direction = Direction::Stopped;
        }
    }
}

pub fn controller_loop(
    elevio_elevator: &elevio::elev::Elevator,
    command_channel_rx: cbc::Receiver<ElevatorRequests>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {
    let rx_channels = inputs::get_input_channels(&elevio_elevator);
    let mut door_timer = timer::Timer::init();

    let mut elevator_state = ElevatorState {
        fsm_state: State::Idle,
        direction: Direction::Stopped,
        obstruction: false, // TODO: Check the obstruction state once in the beginning
        last_floor: Some(0), // TODO: Check floor once in the beginning
        requests: [Request {
            cab: false,
            hall_up: false,
            hall_down: false,
        }; NUMBER_OF_FLOORS],
    };

    loop {
        cbc::select! {
            recv(command_channel_rx) -> command => {
                elevator_state.requests = command.unwrap();

                if elevator_state.fsm_state != State::Idle {
                    continue;
                }

                start_moving(&mut elevator_state, elevio_elevator, &mut door_timer);
            },
            recv(rx_channels.floor_sensor_rx) -> floor => {
                let floor = floor.unwrap();
                info!("Floor: {floor}");

                elevio_elevator.floor_indicator(floor); // Bruk sync lights her kanskje?
                elevator_state.last_floor = Some(floor);

                if elevator_state.fsm_state != State::Moving {
                    continue;
                }

                if elevator_state.should_stop() {
                    info!("Stopping.");
                    elevator_state.fsm_state = State::DoorOpen;
                    elevio_elevator.motor_direction(DIRN_STOP);
                    elevio_elevator.door_light(true);
                    info!("Door open.");
                    door_timer.start(DOOR_OPEN_DURATION);
                }
            },
            recv(rx_channels.stop_button_rx) -> stop_button => {
                let stop_button = stop_button.unwrap();
                info!("Stop button: {:}", stop_button);

                elevio_elevator.motor_direction(DIRN_STOP);

                elevator_state.fsm_state = State::OutOfOrder;
            },
            recv(rx_channels.obstruction_rx) -> obstruction_switch => {
                elevator_state.obstruction = obstruction_switch.unwrap();
                info!("Obstruction: {:}", elevator_state.obstruction);
            },
            recv(door_timer.timeout_channel()) -> _ => {
                if elevator_state.obstruction {
                    door_timer.start(DOOR_OPEN_DURATION);
                    continue;
                }

                elevator_event_tx.send(ElevatorEvent {
                    direction: elevator_state.direction,
                    state: elevator_state.fsm_state,
                    floor: elevator_state.last_floor.unwrap(),
                }).unwrap();

                info!("Door closed.");
                elevio_elevator.door_light(false);

                start_moving(&mut elevator_state, elevio_elevator, &mut door_timer);
            },
        }
    }
}
