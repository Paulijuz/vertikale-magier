use crate::inputs;
use crate::timer;
use crossbeam_channel as cbc;
use driver_rust::elevio;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP};
use std::time::{self, Duration};

const DOOR_OPEN_DURATION: time::Duration = time::Duration::from_secs(3);
const NUMBER_OF_FLOORS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum States {
    Idle,
    Moving,
    DoorOpen,
    OutOfOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Stopped,
}

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub outside_call_up: bool,
    pub outside_call_down: bool,
    pub inside_call: bool,
}

pub type ElevatorOrders = [Order; NUMBER_OF_FLOORS];

pub struct ElevatorEvent {
    pub direction: Direction,
    pub floor: u8,
}

// TODO: Ville kanskje vært bedre om man hadde en "ElevatorController" struct
struct ElevatorState {
    fsm_state: States,
    direction: Direction,
    obstruction: bool,
    last_floor: Option<u8>,
    orders: ElevatorOrders,
}

impl ElevatorState {
    pub fn orders_below(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.orders[..floor as usize]
            .iter()
            .any(|order| order.inside_call || order.outside_call_down || order.outside_call_up)
    }
    pub fn orders_above(&self) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        self.orders[floor as usize + 1..]
            .iter()
            .any(|order| order.inside_call || order.outside_call_down || order.outside_call_up)
    }
    pub fn orders_here(&self, direction: Option<Direction>) -> bool {
        let Some(floor) = self.last_floor else {
            return false;
        };

        let order = self.orders[floor as usize];

        match direction {
            Some(Direction::Up) => order.inside_call || order.outside_call_up,
            Some(Direction::Down) => order.inside_call || order.outside_call_up,
            _ => order.inside_call || order.outside_call_up || order.outside_call_down,
        }
    }
    pub fn next_direction(&self) -> (Direction, States) {
        // TODO: Flytte ut fra main
        match self.direction {
            Direction::Up => {
                return if self.orders_above() {
                    (Direction::Up, States::Moving)
                } else if self.orders_here(None) {
                    (Direction::Down, States::DoorOpen)
                } else if self.orders_below() {
                    (Direction::Down, States::Moving)
                } else {
                    (Direction::Stopped, States::Idle)
                }
            }
            Direction::Down => {
                return if self.orders_below() {
                    (Direction::Down, States::Moving)
                } else if self.orders_here(None) {
                    (Direction::Up, States::DoorOpen)
                } else if self.orders_above() {
                    (Direction::Up, States::Moving)
                } else {
                    (Direction::Stopped, States::Idle)
                }
            }
            Direction::Stopped => {
                return if self.orders_here(None) {
                    (Direction::Stopped, States::DoorOpen)
                } else if self.orders_above() {
                    (Direction::Up, States::Moving)
                } else if self.orders_below() {
                    (Direction::Down, States::Moving)
                } else {
                    (Direction::Stopped, States::Idle)
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
                return self.orders[floor].outside_call_down
                    || self.orders[floor].inside_call
                    || !self.orders_below()
            }
            Direction::Up => {
                return self.orders[floor].outside_call_up
                    || self.orders[floor].inside_call
                    || !self.orders_above()
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

    println!("{direction:?} {state:?}");

    elevator_state.fsm_state = state;

    if state == States::DoorOpen {
        println!("Stopping in move!");
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
    command_channel_rx: cbc::Receiver<ElevatorOrders>,
    elevator_event_tx: cbc::Sender<ElevatorEvent>,
) {
    let rx_channels = inputs::get_input_channels(&elevio_elevator);
    let mut door_timer = timer::Timer::init();

    let mut elevator_state = ElevatorState {
        fsm_state: States::Idle,
        direction: Direction::Stopped,
        obstruction: false, // TODO: Check the obstruction state once in the beginning
        last_floor: Some(0), // TODO: Check floor once in the beginning
        orders: [Order {
            inside_call: false,
            outside_call_up: false,
            outside_call_down: false,
        }; NUMBER_OF_FLOORS],
    };

    loop {
        cbc::select! {
            recv(command_channel_rx) -> command => {
                elevator_state.orders = command.unwrap();

                if elevator_state.fsm_state != States::Idle {
                    continue;
                }

                start_moving(&mut elevator_state, elevio_elevator, &mut door_timer);
            },
            recv(rx_channels.floor_sensor_rx) -> floor => {
                let floor = floor.unwrap();
                println!("Floor: {floor}");

                elevio_elevator.floor_indicator(floor); // Bruk sync lights her kanskje?
                elevator_state.last_floor = Some(floor);

                if elevator_state.fsm_state != States::Moving {
                    continue;
                }

                if elevator_state.should_stop() {
                    println!("Stopping.");
                    elevator_state.fsm_state = States::DoorOpen;
                    elevio_elevator.motor_direction(DIRN_STOP);
                    elevio_elevator.door_light(true);
                    println!("Door open.");
                    door_timer.start(DOOR_OPEN_DURATION);
                }
            },
            recv(rx_channels.stop_button_rx) -> stop_button => {
                let stop_button = stop_button.unwrap();
                println!("Stop button: {:}", stop_button);

                elevio_elevator.motor_direction(DIRN_STOP);

                elevator_state.fsm_state = States::OutOfOrder;
            },
            recv(rx_channels.obstruction_rx) -> obstruction_switch => {
                elevator_state.obstruction = obstruction_switch.unwrap();
                println!("Obstruction: {:}", elevator_state.obstruction);
            },
            recv(door_timer.timeout_channel()) -> _ => {
                if elevator_state.obstruction {
                    door_timer.start(DOOR_OPEN_DURATION);
                    continue;
                }

                elevator_event_tx.send(ElevatorEvent { direction: elevator_state.direction, floor:elevator_state.last_floor.unwrap() }).unwrap();

                println!("Door closed.");
                elevio_elevator.door_light(false);

                start_moving(&mut elevator_state, elevio_elevator, &mut door_timer);
            },
        }
    }
}
