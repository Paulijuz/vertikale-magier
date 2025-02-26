use crate::inputs;
use crate::timer::Timer;
use crossbeam_channel as cbc;
use driver_rust::elevio;
use log::debug;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DOOR_OPEN_DURATION: Duration = Duration::from_secs(3);
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

pub type Requests = [Request; NUMBER_OF_FLOORS];

pub struct ElevatorEvent {
    pub direction: Direction,
    pub state: State,
    pub floor: u8,
}

#[derive(Debug, Clone)]
struct ElevatorController<'e> {
    elevio_driver: &'e elevio::elev::Elevator,
    door_timer: Timer,
    fsm_state: State,
    direction: Direction,
    obstruction: bool,
    last_floor: Option<u8>,
    requests: Requests,
}

impl<'e> ElevatorController<'e> {
    fn new(elevio_driver: &'e elevio::elev::Elevator) -> Self {
        Self {
            elevio_driver,
            door_timer: Timer::init(DOOR_OPEN_DURATION),
            fsm_state: State::Idle,
            direction: Direction::Stopped,
            obstruction: true, // Assume worst until we hear otherwise from driver
            last_floor: Some(0),
            requests: [Request {
                cab: false,
                hall_up: false,
                hall_down: false,
            }; NUMBER_OF_FLOORS],
        }
    }
    fn requests_below(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.requests[..floor as usize]
            .iter()
            .any(|request| request.cab || request.hall_down || request.hall_up)
    }
    fn requests_above(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.requests[floor as usize + 1..]
            .iter()
            .any(|request| request.cab || request.hall_down || request.hall_up)
    }
    fn requests_here(&self, direction: Option<Direction>) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        let request = self.requests[floor as usize];

        match direction {
            Some(Direction::Up) => request.cab || request.hall_up,
            Some(Direction::Down) => request.cab || request.hall_down,
            _ => request.cab || request.hall_up || request.hall_down,
        }
    }
    fn next_direction(&self) -> (Direction, State) {
        match self.direction {
            Direction::Up => {
                return if self.requests_above() {
                    (Direction::Up, State::Moving)
                } else if self.requests_here(Some(Direction::Up)) {
                    (Direction::Up, State::DoorOpen)
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
                } else if self.requests_here(Some(Direction::Down)) {
                    (Direction::Down, State::DoorOpen)
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
    fn should_stop(&self) -> bool {
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
    fn transision_to_moving(&mut self) {
        debug!("Bytter til tilstand \"kjører\".");
        self.fsm_state = State::Moving;

        match self.direction {
            Direction::Up => {
                self.elevio_driver.motor_direction(elevio::elev::DIRN_UP);
                self.direction = Direction::Up;
            }
            Direction::Down => {
                self.elevio_driver.motor_direction(elevio::elev::DIRN_DOWN);
                self.direction = Direction::Down;
            }
            _ => panic!("Prøvde å bytte til tilstand \"kjører\" uten at heisen trenger å kjøre."),
        }
    }
    fn transision_to_door_open(&mut self) {
        debug!("Bytter til tilstand \"dør åpen\".");
        self.fsm_state = State::DoorOpen;

        self.elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
        self.elevio_driver.door_light(true);

        debug!("Dør åpen.");
        self.door_timer.start();
    }
    fn transision_to_idle(&mut self) {
        debug!("Bytter til tilstand \"inaktiv\".");
        self.fsm_state = State::Idle;
    }
}

pub fn controller_loop(
    elevio_elevator: &elevio::elev::Elevator,
    command_channel_rx: cbc::Receiver<Requests>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {
    let rx_channels = inputs::get_input_channels(&elevio_elevator);
    let mut controller = ElevatorController::new(&elevio_elevator);

    loop {
        cbc::select! {
            recv(command_channel_rx) -> command => {
                let requests = command.unwrap();
                debug!("Recieved new requests: {:?}", requests);

                controller.requests = requests;

                if controller.fsm_state != State::Idle {
                    continue;
                }

                let (next_direction, next_state) = controller.next_direction();
                controller.direction = next_direction;

                match next_state {
                    State::DoorOpen => controller.transision_to_door_open(),
                    State::Moving => controller.transision_to_moving(),
                    _ => {},
                }

                if controller.fsm_state != State::Idle {
                    elevator_event_tx.send(ElevatorEvent {
                        direction: controller.direction,
                        state: controller.fsm_state,
                        floor: controller.last_floor.unwrap(),
                    }).unwrap();
                }
            },
            recv(rx_channels.floor_sensor_rx) -> floor => {
                let floor = floor.unwrap();
                debug!("Detekterte etasje: {floor}");

                elevio_elevator.floor_indicator(floor); // TODO: Bruk sync lights her kanskje?
                controller.last_floor = Some(floor);

                if controller.fsm_state != State::Moving {
                    continue;
                }

                if controller.should_stop() {
                    controller.transision_to_door_open();
                }

                elevator_event_tx.send(ElevatorEvent {
                    direction: controller.direction,
                    state: controller.fsm_state,
                    floor: controller.last_floor.unwrap(),
                }).unwrap();
            },
            recv(rx_channels.stop_button_rx) -> stop_button => {
                let stop_button = stop_button.unwrap();
                debug!("Detekterte stopknapp: {:}", stop_button);

                elevio_elevator.motor_direction(elevio::elev::DIRN_STOP);

                controller.fsm_state = State::OutOfOrder;
            },
            recv(rx_channels.obstruction_rx) -> obstruction_switch => {
                controller.obstruction = obstruction_switch.unwrap();
                debug!("Detekterte obstruksjon: {:}", controller.obstruction);
            },
            recv(controller.door_timer.timeout_channel()) -> _ => {
                if controller.obstruction {
                    debug!("Dør obstruert!");
                    controller.door_timer.start();
                    continue;
                }

                elevio_elevator.door_light(false);
                debug!("Dør lukket.");

                let (next_direction, next_state) = controller.next_direction();
                controller.direction = next_direction;
                dbg!(next_direction);

                match next_state {
                    State::DoorOpen => controller.transision_to_door_open(),
                    State::Moving => controller.transision_to_moving(),
                    State::Idle => controller.transision_to_idle(),
                    _ => {},
                }

                elevator_event_tx.send(ElevatorEvent {
                    direction: controller.direction,
                    state: controller.fsm_state,
                    floor: controller.last_floor.unwrap(),
                }).unwrap();
            },
        }
    }
}
