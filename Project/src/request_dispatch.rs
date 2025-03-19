use crossbeam_channel::{self as cbc, tick};
use crossbeam_channel::{select, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::backup::save_state_to_file;
use crate::elevator::controller::Direction;
use crate::elevator::inputs::create_call_button_channel;
use crate::elevator::{controller::ElevatorEvent, lights::set_call_lights};
use crate::network::Node;
use crate::requests::requests::Requests;
use crate::worldview::{HallRequestState, Worldview};

pub fn send_state_to_maser(to_master: &Sender<Worldview>, mut worldview: Worldview) {
    let local_elevator = worldview.local_elevator_state();
    local_elevator.timestamp_last_event = SystemTime::now();
    local_elevator.active = true;
    worldview.iteration += 1;
    to_master.send(worldview).unwrap();
}

//Starting TCP server for Master and distributes incoming orders
pub fn run_dispatcher(
    inital_worldview: Worldview,
    elevio_driver: &Elevator,
    elevator_command_tx: cbc::Sender<Requests>,
    elevator_event_rx: cbc::Receiver<ElevatorEvent>,
) {
    let mut worldview = inital_worldview;
    let ticker = tick(Duration::from_millis(1000));
    let node = Node::<Worldview>::new();
    let call_button_channel = create_call_button_channel(elevio_driver);

    loop {
        select! {
            recv(node.from_master_channel()) -> message => {
                let master_worldview = message.unwrap();

                info!("Received state from master:\n{worldview}");

                worldview.sync_with_master(master_worldview);

                // Send new request list to elevator controller and light controller.
                let requests = worldview.requests_for_local_elevator();

                set_call_lights(&elevio_driver, &requests);
                elevator_command_tx.send(requests).unwrap();
            },
            recv(node.from_slave_channel()) -> message => {
                let slave_worldview = message.unwrap();
                let slave_name = &slave_worldview.name;

                info!("Master received message from \"{slave_name}\":\n{slave_worldview}");

 
                // If we have received a message from a deactivated slave, we can
                // assume that it is alive and activate it again
                if let Some(elevator) = worldview.elevators.get(slave_name) {
                    if !elevator.active {
                        info!("Activating \"{}\" :)", slave_name);
                    }
                } else {
                    info!("New slave connected \"{}\"", slave_name);
                }

                let mut slave_elevator_state = slave_worldview.elevators[slave_name].clone();
                slave_elevator_state.timestamp_last_event = SystemTime::now();
                worldview.elevators.insert(slave_name.clone(), slave_elevator_state);

                if slave_worldview.iteration - worldview.iteration == 1 {
                    // Take new and delete completed orders
                    for (floor, received_request) in slave_worldview.hall_requests.iter().enumerate() {
                        let master_request = worldview.hall_requests[floor].clone();

                        match (&received_request.up, &master_request.up) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => worldview.add_request(floor, Direction::Up),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => worldview.clear_request(floor, Direction::Up),
                            _ => {},
                        }

                        match (&received_request.down, &master_request.down) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => worldview.add_request(floor, Direction::Down),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => worldview.clear_request(floor, Direction::Down),
                            _ => {},
                        }
                    }

                    worldview.assign_requests();
                } else {
                    warn!("Received invalid worldview.");
                }

                worldview.iteration += 1;

                node.to_slaves_channel().send(worldview.clone()).unwrap();
            },
            //Start to inform slaves that master exists
            recv(ticker) -> _ => {
                //Received current timestamp
                let timestamp_start_master_server = SystemTime::now();
                let mut changed = false;

                let elevator_requests: HashMap<_, _> = worldview
                    .elevators
                    .keys()
                    .map(|name| (name.clone(), worldview.requests_for_elevator(name)))
                    .collect();

                //Go through all elevators and get the timestamp for assigned requests.
                for (name, elevator) in &mut worldview.elevators {
                    if let Ok(duration) = timestamp_start_master_server.duration_since(elevator.timestamp_last_event) {
                        let has_orders = elevator_requests[name].unwrap().any_exists();

                        if elevator.active && has_orders && duration > Duration::from_secs(5) {
                            info!("Deactivating {name} :(");
                            elevator.active = false;
                            changed = true;
                        }
                    }
                }

                if changed {
                    worldview.assign_requests();

                    worldview.iteration += 1;

                    //Inform all slaves about new orders
                    node.to_slaves_channel().send(worldview.clone()).unwrap();
                }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                let local_elevator_state = worldview.local_elevator_state();

                //Update state to local elevator
                local_elevator_state.floor = elevator_event.floor;
                local_elevator_state.direction = elevator_event.direction;
                local_elevator_state.behaviour = elevator_event.state;

                //Mark order in floor as completed
                local_elevator_state.cab_requests[elevator_event.floor] = false;

                if elevator_event.direction != Direction::Down {
                    debug!("Cleared up.");
                    worldview.hall_requests[elevator_event.floor].up = HallRequestState::Inactive;
                }
                if elevator_event.direction != Direction::Up {
                    debug!("Cleared down.");
                    worldview.hall_requests[elevator_event.floor].down = HallRequestState::Inactive;
                }

                //Send the updated order list to the elevator controller
                let requests = worldview.requests_for_local_elevator();
                elevator_command_tx.send(requests).unwrap();

                //Inform the master about the new state
                send_state_to_maser(node.to_master_channel(), worldview.clone());
            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;
                let hall_request = &mut worldview.hall_requests[floor];

                //Add order at floor
                match call_button.call {
                    HALL_UP if hall_request.up == HallRequestState::Inactive => hall_request.up = HallRequestState::Requested,
                    HALL_DOWN if hall_request.down == HallRequestState::Inactive => hall_request.down = HallRequestState::Requested,
                    CAB => worldview.local_elevator_state().cab_requests[floor] = true,
                    _ => {},
                }

                //Inform the master about the new state
                send_state_to_maser(node.to_master_channel(), worldview.clone());
            },
        }

        if let Err(e) = save_state_to_file(&worldview, "backup.json") {
            error!("Could not save backup file: {e}");
        }
    }
}
