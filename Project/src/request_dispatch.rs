use crossbeam_channel::{select, tick, Receiver, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{error, info, warn, debug};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime},
};

use crate::network::{Role, Node};
use crate::worldview::RequestStates;
use crate::{backup::save_state_to_file, worldview::system_state_to_string};
use crate::{
    elevator::{
        controller::{ElevatorCommand, ElevatorEvent, ElevatorState},
        inputs::create_call_button_channel,
        lights::{set_cab_lights, set_hall_lights},
        requests::{Direction, RequestType},
    },
    hall_request_assigner::{assign_requests, RequestAssignments},
    worldview::ElevatorView,
};

const DEACTIVATION_POLL: Duration = Duration::from_millis(100);
const ELEVATOR_TIMEOUT: Duration = Duration::from_millis(3500);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    ElevatorState(String, ElevatorState),
    ClearRequests(String, Vec<(usize, RequestType, u32)>),
    NewRequest(String, usize, RequestType),
    RequestStates(RequestStates),
    Acks(String, Vec<(String, usize, RequestType, u32)>),
    RequestAssignments(RequestAssignments),
}

pub fn run_dispatcher(
    name: String,
    node: Node<Message>,
    inital_worldview: RequestStates,
    elevio_driver: &Elevator,
    elevator_command_tx: Sender<ElevatorCommand>,
    elevator_event_rx: Receiver<ElevatorEvent>,
) {
    let mut slave_request_views = inital_worldview.clone();
    let mut master_request_views = inital_worldview;
    let mut slave_request_assignments = RequestAssignments::new(elevio_driver.num_floors as usize);
    let mut master_request_assignments = RequestAssignments::new(elevio_driver.num_floors as usize);
    let mut elevator_views: HashMap<String, ElevatorView> = HashMap::new();
    let mut newest_elevator_state: Option<ElevatorState> = None;
    let mut role: Role = Role::Master(HashSet::new());

    let request_timeout_ticker = tick(DEACTIVATION_POLL);
    let call_button_channel = create_call_button_channel(elevio_driver);

    set_cab_lights(&elevio_driver, &slave_request_views.cab_requests_as_bools(&name));
    set_hall_lights(&elevio_driver, &slave_request_views.hall_requests_as_bools());

    loop {
        select! {
            recv(node.from_master_channel()) -> message => {
                let message = message.unwrap();

                match message {
                    Message::RequestStates(new_request_views) => {
                        // debug!("Received state from master \"{}\":\n{:?}", name, new_request_views);
                        slave_request_views = new_request_views;

                        let not_acked = slave_request_views.not_acked(&name);

                        if not_acked.len() > 0 {
                            node.to_master_channel().send(Message::Acks(name.clone(), not_acked)).unwrap();
                        }

                        set_cab_lights(&elevio_driver, &slave_request_views.cab_requests_as_bools(&name));
                        set_hall_lights(&elevio_driver, &slave_request_views.hall_requests_as_bools());
                    },
                    Message::RequestAssignments(new_request_assignments) => {
                        // debug!("Received request assignments from master \"{}\":\n{:?}", name, new_request_assignments);
                        slave_request_assignments = new_request_assignments;

                        for (active, floor, request_type) in slave_request_assignments.requests(&name) {
                            if active {
                                elevator_command_tx.send(ElevatorCommand::AddRequest(floor, request_type)).unwrap();
                            } else {
                                elevator_command_tx.send(ElevatorCommand::ClearRequest(floor, request_type)).unwrap();
                            }
                        }
                    },
                    invalid_message => {
                        warn!("Received invalid message from master: {invalid_message:?}");
                        continue;
                    },
                }

                master_request_views = slave_request_views.clone();
                master_request_assignments = slave_request_assignments.clone();

                info!("{}\n", system_state_to_string(&slave_request_views, &slave_request_assignments, &elevator_views));
            },
            recv(node.from_slave_channel()) -> message => {
                let message = message.unwrap();

                let Role::Master(connected_nodes) = &role else {
                    warn!("Received message from slave while being a master: {message:?}");
                    continue;
                };

                info!("Received message from slave: {message:?}");

                match message {
                    Message::ElevatorState(name, state) => {
                        // If we have received a message from a deactivated slave, we can
                        // assume that it is alive and activate it again
                        if let Some(elevator) = elevator_views.get_mut(&name) {
                            elevator.state = state;
                            elevator.timestamp_last_event = SystemTime::now();
                        } else {
                            info!("New slave \"{}\" connected.", name);
                            elevator_views.insert(name, ElevatorView {
                                active: false,
                                state,
                                timestamp_last_event: SystemTime::now(),
                            });
                        }
                    },
                    Message::NewRequest(name, floor, request_type) => {
                        if !master_request_views.set_pending(floor, name, request_type) {
                            warn!("Failed to set request to pending.");
                            continue;
                        }
                    },
                    Message::ClearRequests(name, requests) => {
                        let mut changed = false;

                        for (floor, request_type, iteration) in requests {
                            changed |= master_request_views.set_inactive(floor, name.clone(), request_type, iteration);
                        }

                        if !changed {
                            warn!("Failed to clear requests.");
                            continue;
                        }

                        // If we recieve a clear floor from an elevator we can be sure it's alive.
                        if let Some(elevator) = elevator_views.get_mut(&name) {
                            elevator.timestamp_last_event = SystemTime::now();
                        }

                    },
                    Message::Acks(name, requests) => {
                        let mut changed = false;

                        for (request_name, floor, request_type, iteration_check) in requests{
                            changed |= master_request_views.add_ack(floor, request_name, request_type, name.clone(), iteration_check);
                        }

                        if !changed {
                            warn!("Failed to acknowledge.");
                            continue;
                        }
                    }
                    _ => {},
                }

                if master_request_views.set_all_acked_active(&connected_nodes) {
                    match assign_requests(&master_request_views, &elevator_views) {
                        Some(new_request_assignment) => master_request_assignments = new_request_assignment,
                        None => error!("Could not assign requests."),
                    }

                    node.to_slaves_channel().send(Message::RequestAssignments(master_request_assignments.clone())).unwrap();
                }

                node.to_slaves_channel().send(Message::RequestStates(master_request_views.clone())).unwrap();
            },
            recv(node.connection_update_channel()) -> connection_update => {
                let new_role = connection_update.unwrap();

                if role != new_role {
                    role = new_role;

                    match &role {
                        Role::Master(connected_nodes) => {
                            info!("Connected nodes: {connected_nodes:?}");
                        },
                        Role::Slave => if let Some(elevator_state) = newest_elevator_state {
                            node.to_master_channel().send(Message::ElevatorState(name.clone(), elevator_state)).unwrap();
                        },
                    }
                }
            },
            //Start to inform slaves that master exists
            recv(request_timeout_ticker) -> _ => {
                if role == Role::Slave {
                    continue;
                }

                //Received current timestamp
                let now = SystemTime::now();
                let mut changed = false;

                for (name, elevator) in &mut elevator_views {
                    let Ok(duration) = now.duration_since(elevator.timestamp_last_event) else {
                        continue;
                    };

                    if elevator.active && master_request_assignments.has_assignment(name) && duration > ELEVATOR_TIMEOUT {
                        info!("Deactivating {name}. :(");
                        // elevator.active = false;
                        // changed = true;
                    } else if !elevator.active && duration < ELEVATOR_TIMEOUT {
                        info!("Activating {name}. :)");
                        elevator.active = true;
                        // changed = true;
                    }
                }

                if changed {
                    match assign_requests(&master_request_views, &elevator_views) {
                        Some(new_request_assignment) => master_request_assignments = new_request_assignment,
                        None => error!("Could not assign requests."),
                    }

                    node.to_slaves_channel().send(Message::RequestAssignments(master_request_assignments.clone())).unwrap();
                }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                let message = match elevator_event {
                    ElevatorEvent::RequestsCleared(requests) => {
                        Message::ClearRequests(name.clone(), requests.iter().map(|&(floor, request_type)| (floor, request_type, slave_request_views.iteration(floor, &name, request_type))).collect())
                    },
                    ElevatorEvent::StateUpdated(state) => {
                        newest_elevator_state = Some(state.clone());
                        Message::ElevatorState(name.clone(), state)
                    },
                };

                //Inform the master about the new state
                node.to_master_channel().send(message).unwrap();
            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();
                let floor = call_button.floor as usize;

                let request_type = match call_button.call {
                    HALL_UP => RequestType::Hall(Direction::Up),
                    HALL_DOWN => RequestType::Hall(Direction::Down),
                    CAB => RequestType::Cab,
                    unknown_call => {
                        warn!("Received unkown call button: {unknown_call}");
                        continue;
                    },
                };

                node.to_master_channel().send(Message::NewRequest(name.clone(), floor, request_type)).unwrap();
            },
        }

        if let Err(e) = save_state_to_file(&master_request_views, "backup.json") {
            error!("Could not save backup file: {e}");
        }
    }
}
