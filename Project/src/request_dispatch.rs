use crossbeam_channel::{select, tick, Receiver, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{debug, error, info, warn};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use crate::backup::save_state_to_file;
use crate::elevator::{
    controller::{Direction, ElevatorEvent},
    inputs::create_call_button_channel,
    lights::{set_cab_lights, set_hall_lights},
};
use crate::network::Node;
use crate::requests::requests::Requests;
use crate::worldview::{HallRequestState, Worldview};

const ELEVATOR_TIMEOUT: Duration = Duration::from_millis(3500);

pub fn run_dispatcher(
    node: Node<Worldview>,
    inital_worldview: Worldview,
    elevio_driver: &Elevator,
    elevator_command_tx: Sender<Requests>,
    elevator_event_rx: Receiver<ElevatorEvent>,
) {
    let mut global_worldview = inital_worldview.clone();
    let mut local_worldview = inital_worldview;
    let deactivation_ticker = tick(Duration::from_millis(1000));
    let call_button_channel = create_call_button_channel(elevio_driver);

    loop {
        select! {
            recv(node.from_master_channel()) -> message => {
                global_worldview = message.unwrap();

                info!("Received state from master \"{}\":\n{}", global_worldview.name, global_worldview);

                // Sync with the master and send new requests to the elevator controller
                local_worldview.sync_with_master(global_worldview.clone());
                let requests = local_worldview.requests_for_local_elevator();

                set_cab_lights(&elevio_driver, &requests);
                set_hall_lights(&elevio_driver, &local_worldview.hall_requests);

                elevator_command_tx.send(requests).unwrap();

                if local_worldview.name != global_worldview.name && local_worldview.hall_requests.iter().any(|r| r.up == HallRequestState::Requested || r.down == HallRequestState::Requested) {
                    node.to_master_channel().send(local_worldview.clone()).unwrap();
                }
            },
            recv(node.from_slave_channel()) -> message => {
                let mut slave_worldview = message.unwrap();
                let mut slave_elevator_state = slave_worldview.local_elevator_state().clone();
                let slave_name = &slave_worldview.name;

                info!("Master received message from \"{slave_name}\":\n{slave_worldview}");

                let mut master_worldview = global_worldview.clone();
                master_worldview.name = local_worldview.name.clone();

                // If we have received a message from a deactivated slave, we can
                // assume that it is alive and activate it again
                if let Some(elevator) = master_worldview.elevators.get_mut(slave_name) {
                    if !elevator.active {
                        info!("Aktiverer \"{}\" :)", slave_name);
                        elevator.active = true;
                    }
                } else {
                    info!("New slave connected \"{}\"", slave_name);
                }

                slave_elevator_state.timestamp_last_event = SystemTime::now();
                master_worldview.elevators.insert(slave_name.clone(), slave_elevator_state);

                if slave_worldview.iteration != master_worldview.iteration {
                    warn!("Received invalid worldview. ({} != {})", slave_worldview.iteration, master_worldview.iteration);
                    node.to_slaves_channel().send(global_worldview.clone()).unwrap();
                    continue;
                }

                // Take new and delete completed orders
                for (floor, received_request) in slave_worldview.hall_requests.iter().enumerate() {
                    let master_request = master_worldview.hall_requests[floor].clone();

                    match (&received_request.up, &master_request.up) {
                        (HallRequestState::Requested, HallRequestState::Inactive) => master_worldview.add_request(floor, Direction::Up),
                        (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_worldview.clear_request(floor, Direction::Up),
                        _ => {},
                    }

                    match (&received_request.down, &master_request.down) {
                        (HallRequestState::Requested, HallRequestState::Inactive) => master_worldview.add_request(floor, Direction::Down),
                        (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_worldview.clear_request(floor, Direction::Down),
                        _ => {},
                    }
                }

                // If the slave is also the master, only add requests, Don't assign them until we hear from another slave
                // about the requests. That way we guaranteee that at least two nodes know about the states.
                if master_worldview.name != slave_worldview.name {
                    master_worldview.assign_requests();
                }

                master_worldview.iteration += 1;

                // Send the worldview to all slaves. Slaves will repeat the message back to us as a form of ack.
                node.to_slaves_channel().send(master_worldview.clone()).unwrap();

                // Only update the global worldview if the slave which we received the update from is not also the master.
                // This is requried because at least two nodes need to know about a worldview to guarantee that nothing is lost.
                if master_worldview.name != slave_worldview.name {
                    global_worldview = master_worldview
                }
            },
            //Start to inform slaves that master exists
            recv(deactivation_ticker) -> _ => {
                //Received current timestamp
                let timestamp_start_master_server = SystemTime::now();
                let mut changed = false;

                let elevator_requests: HashMap<_, _> = global_worldview
                    .elevators
                    .keys()
                    .map(|name| (name.clone(), global_worldview.requests_for_elevator(name)))
                    .collect();

                //Go through all elevators and get the timestamp for assigned requests.
                for (name, elevator) in &mut global_worldview.elevators {
                    if let Ok(duration) = timestamp_start_master_server.duration_since(elevator.timestamp_last_event) {
                        let has_orders = elevator_requests[name].as_ref().unwrap().any_exists();

                        if elevator.active && has_orders && duration > ELEVATOR_TIMEOUT {
                            info!("Deactivating {name} :(");
                            elevator.active = false;
                            changed = true;
                        }
                    }
                }

                if changed {
                    global_worldview.assign_requests();

                    global_worldview.iteration += 1;

                    //Inform all slaves about new orders
                    global_worldview.name = local_worldview.name.clone();
                    node.to_slaves_channel().send(global_worldview.clone()).unwrap();
                }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                let local_elevator = local_worldview.local_elevator_state();

                match elevator_event {
                    ElevatorEvent::FloorCleared((floor, direction)) => {
                        local_elevator.cab_requests[floor] = false;

                        if direction != Direction::Down {
                            debug!("Cleared up.");
                            local_worldview.hall_requests[floor].up = HallRequestState::Inactive;
                        }
                        if direction != Direction::Up {
                            debug!("Cleared down.");
                            local_worldview.hall_requests[floor].down = HallRequestState::Inactive;
                        }

                        //Send the updated order list to the elevator controller
                        let requests = local_worldview.requests_for_local_elevator();
                        elevator_command_tx.send(requests).unwrap();
                    },
                    ElevatorEvent::StateUpdated(state) => {
                        local_elevator.state = state;
                    },
                }

                //Inform the master about the new state
                node.to_master_channel().send(local_worldview.clone()).unwrap();
            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;
                let hall_request = &mut local_worldview.hall_requests[floor];

                //Add order at floor
                match call_button.call {
                    HALL_UP if hall_request.up == HallRequestState::Inactive => hall_request.up = HallRequestState::Requested,
                    HALL_DOWN if hall_request.down == HallRequestState::Inactive => hall_request.down = HallRequestState::Requested,
                    CAB => local_worldview.local_elevator_state().cab_requests[floor] = true,
                    _ => {},
                }

                //Inform the master about the new state
                node.to_master_channel().send(local_worldview.clone()).unwrap();
            },
        }

        if let Err(e) = save_state_to_file(&local_worldview, "backup.json") {
            error!("Could not save backup file: {e}");
        }
    }
}
