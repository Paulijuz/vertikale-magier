use std::time::Duration;

use crossbeam_channel as cbc;
use driver_rust::elevio;
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    requests::requests::Requests,
    timer::Timer,
};

use super::inputs::{
    create_floor_sensor_channel, create_obstruction_channel, create_stop_button_channel,
};

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

pub struct ElevatorEvent {
    pub direction: Direction,
    pub state: Behaviour,
    pub floor: usize,
}

#[derive(Debug, Clone)]
struct ElevatorController<'e> {
    elevio_driver: &'e elevio::elev::Elevator,
    door_timer: Timer,
    behaviour: Behaviour,
    direction: Direction,
    obstruction: bool,
    last_floor: Option<usize>,
    requests: Requests,
}

impl<'e> ElevatorController<'e> {
    fn new(elevio_driver: &'e elevio::elev::Elevator) -> Self {
        Self {
            elevio_driver,
            door_timer: Timer::init(DOOR_OPEN_DURATION),
            behaviour: Behaviour::Idle,
            direction: Direction::Stopped,
            obstruction: true, // Assume worst until we hear otherwise from driver
            last_floor: Some(0),
            requests: Default::default(),
        }
    }

    fn next_direction(&self) -> (Direction, Behaviour) {
        let floor = self
            .last_floor
            .expect("Called next direction without known floor.");

        match self.direction {
            Direction::Up => {
                return if self.requests.any_above_floor(floor) {
                    (Direction::Up, Behaviour::Moving)
                } else if self.requests.up_at_floor(floor) {
                    (Direction::Up, Behaviour::DoorOpen)
                } else if self.requests.any_at_floor(floor) {
                    (Direction::Down, Behaviour::DoorOpen)
                } else if self.requests.any_below_floor(floor) {
                    (Direction::Down, Behaviour::Moving)
                } else {
                    (Direction::Stopped, Behaviour::Idle)
                }
            }
            Direction::Down => {
                return if self.requests.any_below_floor(floor) {
                    (Direction::Down, Behaviour::Moving)
                } else if self.requests.down_at_floor(floor) {
                    (Direction::Down, Behaviour::DoorOpen)
                } else if self.requests.any_at_floor(floor) {
                    (Direction::Up, Behaviour::DoorOpen)
                } else if self.requests.any_above_floor(floor) {
                    (Direction::Up, Behaviour::Moving)
                } else {
                    (Direction::Stopped, Behaviour::Idle)
                }
            }
            Direction::Stopped => {
                return if self.requests.any_at_floor(floor) {
                    (Direction::Stopped, Behaviour::DoorOpen)
                } else if self.requests.any_above_floor(floor) {
                    (Direction::Up, Behaviour::Moving)
                } else if self.requests.any_below_floor(floor) {
                    (Direction::Down, Behaviour::Moving)
                } else {
                    (Direction::Stopped, Behaviour::Idle)
                }
            }
        }
    }
    fn should_stop(&self) -> bool {
        let floor = self
            .last_floor
            .expect("Called next direction without known floor.");

        match self.direction {
            Direction::Down => {
                return self.requests.down_at_floor(floor)
                    || !self.requests.any_below_floor(floor)
            }
            Direction::Up => {
                return self.requests.any_at_floor(floor)
                    || !self.requests.any_above_floor(floor)
            }
            Direction::Stopped => return true,
        }
    }
    fn transision_to_moving(&mut self) {
        debug!("Changing to state \"moving\".");
        self.behaviour = Behaviour::Moving;

        match self.direction {
            Direction::Up => {
                self.elevio_driver.motor_direction(elevio::elev::DIRN_UP);
                self.direction = Direction::Up;
            }
            Direction::Down => {
                self.elevio_driver.motor_direction(elevio::elev::DIRN_DOWN);
                self.direction = Direction::Down;
            }
            _ => panic!("Tried to change to state \"moving\" without elevator having to move."),
        }
    }
    fn transision_to_door_open(&mut self) {
        debug!("Changing to state \"door open\".");
        self.behaviour = Behaviour::DoorOpen;

        self.elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
        self.elevio_driver.door_light(true);

        debug!("Door open.");
        self.door_timer.start();
    }
    fn transision_to_idle(&mut self) {
        debug!("Changing to state \"inactive\".");
        self.behaviour = Behaviour::Idle;
    }
}

pub fn controller_loop(
    elevio_driver: &elevio::elev::Elevator,
    command_channel_rx: cbc::Receiver<Requests>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {
    let floor_sensor_channel = create_floor_sensor_channel(elevio_driver);
    let obstruction_channel = create_obstruction_channel(elevio_driver);
    let stop_button_channel = create_stop_button_channel(elevio_driver);
    let mut controller = ElevatorController::new(elevio_driver);

    loop {
        cbc::select! {
            recv(command_channel_rx) -> command => {
                let requests = command.unwrap();
                debug!("Recieved new requests: {:?}", requests);

                controller.requests = requests;
                debug!("{:?}", controller.behaviour);
                if controller.behaviour != Behaviour::Idle {
                    continue;
                }

                let (next_direction, next_state) = controller.next_direction();
                controller.direction = next_direction;

                match next_state {
                    Behaviour::DoorOpen => controller.transision_to_door_open(),
                    Behaviour::Moving => controller.transision_to_moving(),
                    _ => {},
                }

                if controller.behaviour != Behaviour::Idle {
                    elevator_event_tx.send(ElevatorEvent {
                        direction: controller.direction,
                        state: controller.behaviour,
                        floor: controller.last_floor.unwrap(),
                    }).unwrap();
                }
            },
            recv(floor_sensor_channel) -> floor => {
                let floor = floor.unwrap();
                debug!("Detected floor: {floor}");

                elevio_driver.floor_indicator(floor); // TODO: Bruk sync lights her kanskje?
                controller.last_floor = Some(floor as usize);

                if controller.behaviour != Behaviour::Moving {
                    continue;
                }

                if controller.should_stop() {
                    controller.transision_to_door_open();
                }

                elevator_event_tx.send(ElevatorEvent {
                    direction: controller.direction,
                    state: controller.behaviour,
                    floor: controller.last_floor.unwrap(),
                }).unwrap();
            },
            recv(stop_button_channel) -> stop_button => {
                let stop_button = stop_button.unwrap();
                debug!("Detected stop-button: {:}", stop_button);

                if !stop_button {
                    continue;
                }

                elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
                controller.behaviour = Behaviour::OutOfOrder;
            },
            recv(obstruction_channel) -> obstruction_switch => {
                controller.obstruction = obstruction_switch.unwrap();
                debug!("Detected obstruction: {:}", controller.obstruction);
            },
            recv(controller.door_timer.timeout_channel()) -> _ => {
                if controller.obstruction {
                    debug!("Door obstructed!");
                    controller.door_timer.start();
                    continue;
                }

                elevio_driver.door_light(false);
                debug!("Door closed.");

                let (next_direction, next_state) = controller.next_direction();
                controller.direction = next_direction;
                dbg!(next_direction);

                match next_state {
                    Behaviour::DoorOpen => controller.transision_to_door_open(),
                    Behaviour::Moving => controller.transision_to_moving(),
                    Behaviour::Idle => controller.transision_to_idle(),
                    _ => {},
                }

                elevator_event_tx.send(ElevatorEvent {
                    direction: controller.direction,
                    state: controller.behaviour,
                    floor: controller.last_floor.unwrap(),
                }).unwrap();
            },
        }
    }
}
