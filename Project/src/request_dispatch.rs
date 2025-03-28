use crossbeam_channel::{select, tick, Receiver, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use crate::{backup::save_state_to_file, worldview::requests_with_assignments_to_string};
use crate::network::Node;
use crate::worldview::RequestStates;
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

const ELEVATOR_TIMEOUT: Duration = Duration::from_millis(3500);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DispatcherMessage {
    ElevatorState(String, ElevatorState),
    ClearRequest(String, usize, RequestType),
    NewRequest(String, usize, RequestType),
    RequestStates(String, RequestStates),
    RequestAssignments(String, RequestAssignments),
}

pub fn run_dispatcher(
    name: String,
    node: Node<DispatcherMessage>,
    inital_worldview: RequestStates,
    elevio_driver: &Elevator,
    elevator_command_tx: Sender<ElevatorCommand>,
    elevator_event_rx: Receiver<ElevatorEvent>,
) {
    let mut request_views = inital_worldview.clone();
    let mut master_request_assignments = RequestAssignments::new(inital_worldview.hall_requests.len());
    let mut slave_request_assignments = RequestAssignments::new(inital_worldview.hall_requests.len());
    let mut elevator_views: HashMap<String, ElevatorView> = HashMap::new();

    let deactivation_ticker = tick(Duration::from_millis(1000));
    let call_button_channel = create_call_button_channel(elevio_driver);

    loop {
        select! {
            recv(node.from_master_channel()) -> message => {
                let message = message.unwrap();

                match message {
                    DispatcherMessage::RequestStates(name, new_request_views) => {
                        // info!("Received state from master \"{}\":\n{}", name, new_request_views);
                        request_views = new_request_views;

                        set_cab_lights(&elevio_driver, &request_views.cab_requests_as_bools(&name));
                        set_hall_lights(&elevio_driver, &request_views.hall_requests_as_bools());
                    },
                    DispatcherMessage::RequestAssignments(name, new_request_assignments) => {
                        // info!("Received request assignments from master \"{}\":\n{:?}", name, new_request_assignments);

                        for (active, floor, request_type) in slave_request_assignments.different_requests(&new_request_assignments, &name) {
                            if active {
                                error!("{floor} {request_type:?}");
                                elevator_command_tx.send(ElevatorCommand::AddRequest(floor, request_type)).unwrap();
                            } else {
                                elevator_command_tx.send(ElevatorCommand::ClearRequest(floor, request_type)).unwrap();
                            }
                        }

                        slave_request_assignments = new_request_assignments.clone();
                        master_request_assignments = new_request_assignments;
                    },
                    invalid_message => {
                        warn!("Received invalid message from master: {invalid_message:?}");
                        continue;
                    },
                }

                info!("{}\n", requests_with_assignments_to_string(&request_views, &elevator_views));
            },
            recv(node.from_slave_channel()) -> message => {
                let message = message.unwrap();

                info!("Received message from slave: {message:?}");

                match message {
                    DispatcherMessage::ElevatorState(name, state) => {
                        // If we have received a message from a deactivated slave, we can
                        // assume that it is alive and activate it again
                        if let Some(elevator) = elevator_views.get_mut(&name) {
                            if !elevator.active {
                                info!("Activating \"{}\" :)", name);
                            }
                        } else {
                            info!("New slave \"{}\" connected.", name);
                        }

                        elevator_views.insert(name, ElevatorView {
                            active: true,
                            state,
                            timestamp_last_event: SystemTime::now(),
                        });
                    },
                    DispatcherMessage::NewRequest(name, floor, request_type) => {
                        request_views.set_pending(floor, name, request_type);
                    },
                    DispatcherMessage::ClearRequest(name, floor, request_type) => {
                        request_views.set_inactive(floor, name, request_type);
                    },
                    _ => {},
                }

                // if slave_worldview.iteration != master_worldview.iteration {
                //     warn!("Received invalid worldview. ({} != {})", slave_worldview.iteration, master_worldview.iteration);
                //     node.to_slaves_channel().send(DispatcherMessage::Synchronize(global_worldview.clone())).unwrap();
                //     continue;
                // }
                request_views.actiave_all_confirmed();

                match assign_requests(&request_views, &elevator_views) {
                    Some(new_request_assignment) => master_request_assignments = new_request_assignment,
                    None => error!("Could not assign requests."),
                }

                node.to_slaves_channel().send(DispatcherMessage::RequestStates(name.clone(), request_views.clone())).unwrap();
                node.to_slaves_channel().send(DispatcherMessage::RequestAssignments(name.clone(), master_request_assignments.clone())).unwrap();
            },
            //Start to inform slaves that master exists
            recv(deactivation_ticker) -> _ => {
                //Received current timestamp
                let now = SystemTime::now();
                let mut changed = false;

                for (name, elevator) in &mut elevator_views {
                    if !elevator.active {
                        continue;
                    }

                    let Ok(duration) = now.duration_since(elevator.timestamp_last_event) else {
                        continue;
                    };

                    if master_request_assignments.has_assignment(name) && duration > ELEVATOR_TIMEOUT {
                        info!("Deactivating {name}. :(");
                        elevator.active = false;
                        changed = true;
                    }
                }

                if changed {
                    // request_views.assign_requests(&elevator_views);
                    // request_views.iteration += 1;

                    //Inform all slaves about new orders
                    // node.to_slaves_channel().send(DispatcherMessage::RequestStates(name.clone(), request_views.clone())).unwrap();
                }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                let message = match elevator_event {
                    ElevatorEvent::RequestCleared(floor, request_type) => {
                        DispatcherMessage::ClearRequest(name.clone(), floor, request_type)
                    },
                    ElevatorEvent::StateUpdated(state) => {
                        DispatcherMessage::ElevatorState(name.clone(), state)
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

                node.to_master_channel().send(DispatcherMessage::NewRequest(name.clone(), floor, request_type)).unwrap();
            },
        }

        if let Err(e) = save_state_to_file(&request_views, "backup.json") {
            error!("Could not save backup file: {e}");
        }
    }
}
