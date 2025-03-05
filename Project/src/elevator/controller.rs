use std::time::Duration;

use crossbeam_channel as cbc;
use driver_rust::elevio;
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::{
    requests::requests::{
        requests_above_floor, requests_at_floor, requests_below_floor, Direction, Requests,
    },
    timer::Timer,
};

use super::inputs::{
    self, create_floor_sensor_channel, create_obstruction_channel, create_stop_button_channel,
};

const DOOR_OPEN_DURATION: Duration = Duration::from_secs(3);

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
    pub floor: u8,
}

#[derive(Debug, Clone)]
struct ElevatorController<'e> {
    elevio_driver: &'e elevio::elev::Elevator,
    door_timer: Timer,
    behaviour: Behaviour,
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
            .expect("Called next direction without known floor.") as usize;

        match self.direction {
            Direction::Up => {
                return if requests_above_floor(&self.requests, floor) {
                    (Direction::Up, Behaviour::Moving)
                } else if requests_at_floor(&self.requests, floor, Some(Direction::Up)) {
                    (Direction::Up, Behaviour::DoorOpen)
                } else if requests_at_floor(&self.requests, floor, None) {
                    (Direction::Down, Behaviour::DoorOpen)
                } else if requests_below_floor(&self.requests, floor) {
                    (Direction::Down, Behaviour::Moving)
                } else {
                    (Direction::Stopped, Behaviour::Idle)
                }
            }
            Direction::Down => {
                return if requests_below_floor(&self.requests, floor) {
                    (Direction::Down, Behaviour::Moving)
                } else if requests_at_floor(&self.requests, floor, Some(Direction::Down)) {
                    (Direction::Down, Behaviour::DoorOpen)
                } else if requests_at_floor(&self.requests, floor, None) {
                    (Direction::Up, Behaviour::DoorOpen)
                } else if requests_above_floor(&self.requests, floor) {
                    (Direction::Up, Behaviour::Moving)
                } else {
                    (Direction::Stopped, Behaviour::Idle)
                }
            }
            Direction::Stopped => {
                return if requests_at_floor(&self.requests, floor, None) {
                    (Direction::Stopped, Behaviour::DoorOpen)
                } else if requests_above_floor(&self.requests, floor) {
                    (Direction::Up, Behaviour::Moving)
                } else if requests_below_floor(&self.requests, floor) {
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
            .expect("Called next direction without known floor.") as usize;

        match self.direction {
            Direction::Down => {
                return self.requests[floor].hall_down
                    || self.requests[floor].cab
                    || !requests_below_floor(&self.requests, floor)
            }
            Direction::Up => {
                return self.requests[floor].hall_up
                    || self.requests[floor].cab
                    || !requests_above_floor(&self.requests, floor)
            }
            Direction::Stopped => return true,
        }
    }
    fn transision_to_moving(&mut self) {
        debug!("Bytter til tilstand \"kjører\".");
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
            _ => panic!("Prøvde å bytte til tilstand \"kjører\" uten at heisen trenger å kjøre."),
        }
    }
    fn transision_to_door_open(&mut self) {
        debug!("Bytter til tilstand \"dør åpen\".");
        self.behaviour = Behaviour::DoorOpen;

        self.elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
        self.elevio_driver.door_light(true);

        debug!("Dør åpen.");
        self.door_timer.start();
    }
    fn transision_to_idle(&mut self) {
        debug!("Bytter til tilstand \"inaktiv\".");
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
                debug!("Detekterte etasje: {floor}");

                elevio_driver.floor_indicator(floor); // TODO: Bruk sync lights her kanskje?
                controller.last_floor = Some(floor);

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
                debug!("Detekterte stopknapp: {:}", stop_button);

                if !stop_button {
                    continue;
                }

                elevio_driver.motor_direction(elevio::elev::DIRN_STOP);
                controller.behaviour = Behaviour::OutOfOrder;
            },
            recv(obstruction_channel) -> obstruction_switch => {
                controller.obstruction = obstruction_switch.unwrap();
                debug!("Detekterte obstruksjon: {:}", controller.obstruction);
            },
            recv(controller.door_timer.timeout_channel()) -> _ => {
                if controller.obstruction {
                    debug!("Dør obstruert!");
                    controller.door_timer.start();
                    continue;
                }

                elevio_driver.door_light(false);
                debug!("Dør lukket.");

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
