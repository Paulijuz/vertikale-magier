use crossbeam_channel::{select, tick, Receiver, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{warn, info};
use std::{collections::HashMap, time::Duration};

use crate::{elevator::{
    controller::{ElevatorDirection, ElevatorEvent, ElevatorState},
    inputs::create_call_button_channel,
    lights::{set_cab_lights, set_hall_lights},
}, message::Message, requests::{cab::CabRequests, hall::{HallRequestDirection, HallRequests}}};
use crate::network::Node;
use crate::requests::local::LocalRequests;

// #[derive(Debug, Clone, Serialize, Deserialize)]
// enum DispatcherMessage {
//     NewRequest((usize, /* TODO: Request Type */)),
//     DispatchRequest((u32, usize)),
//     ClearRequest((u32, usize, /* TODO: Direction */)),
//     ElevatorState(ElevatorState),
// }

pub fn run_dispatcher(
    name: String,
    node: Node<Message>,
    elevio_driver: &Elevator,
    elevator_command_tx: Sender<LocalRequests>,
    elevator_event_rx: Receiver<ElevatorEvent>,
) {
    // let mut global_worldview = Worldview::new(String::from(""), elevio_driver.num_floors as usize);
    // let mut local_worldview = inital_worldview;
    let deactivation_ticker = tick(Duration::from_millis(1000));
    let call_button_channel = create_call_button_channel(elevio_driver);

    let mut elevator_states = HashMap::<String, ElevatorState>::new();
    let mut cab_requests = CabRequests::new(elevio_driver.num_floors as usize);
    let mut hall_requests = HallRequests::new(elevio_driver.num_floors as usize);
    let mut hall_request_assignments = 0; // TODO

    loop {
        select! {
            recv(node.from_master_channel()) -> message => {
                let message = message.unwrap();

                info!("Received message from master."); // TODO: Add message and name

                match message {
                    Message::NewHallRequest(floor, direciotn) => {
                    
                    },
                    Message::NewCabRequest(floor, name) => {

                    },
                    Message::ClearHallRequest(floor, directon, iteration) => {

                    },
                    Message::ClearCabRequest(floor, name, iteration) => {

                    }
                    _ => continue,
                }

                set_cab_lights(&elevio_driver, &cab_requests.as_bools(&name));
                set_hall_lights(&elevio_driver, &hall_requests.as_bools());

                elevator_command_tx.send(LocalRequests::from_vectors(cab_requests.as_bools(&name), &hall_requests.as_bools())).unwrap();
            },
            recv(node.from_slave_channel()) -> message => {
                let message = message.unwrap();

                match message {
                    Message::NewHallRequest(floor, direction) => {
                        hall_requests.set_pending(floor, direction);
                    },
                    Message::NewCabRequest(floor, name) => {
                        cab_requests.set_pending(floor, name)
                    },
                    Message::ClearHallRequest(floor, direction, iteration) => {
                        // TODO: check iteration
                        hall_requests.set_inactive(floor, direction);
                    },
                    Message::ClearCabRequest(floor, name, iteration) => {
                        // TODO: check iteration
                        cab_requests.set_inactive(floor, name);
                    }
                    Message::ElevatorState(name, state) => {
                        // TODO: Add timestamp
                        elevator_states.insert(name, state);
                    },
                    _ => continue,
                }

                // TODO: Assign requests

                // Send the worldview to all slaves. Slaves will repeat the message back to us as a form of ack.
                // node.to_slaves_channel().send(master_worldview.clone()).unwrap();
            },
            //Start to inform slaves that master exists
            recv(deactivation_ticker) -> _ => {
                //Received current timestamp
                // let timestamp_start_master_server = SystemTime::now();
                // let mut changed = false;

                // let elevator_requests: HashMap<_, _> = global_worldview
                //     .elevators
                //     .keys()
                //     .map(|name| (name.clone(), global_worldview.requests_for_elevator(name)))
                //     .collect();

                // //Go through all elevators and get the timestamp for assigned requests.
                // for (name, elevator) in &mut global_worldview.elevators {
                //     if let Ok(duration) = timestamp_start_master_server.duration_since(elevator.timestamp_last_event) {
                //         let has_orders = elevator_requests[name].as_ref().unwrap().any_exists();

                //         if elevator.active && has_orders && duration > Duration::from_secs(5) {
                //             info!("Deactivating {name} :(");
                //             elevator.active = false;
                //             changed = true;
                //         }
                //     }
                // }

                // if changed {
                //     global_worldview.assign_requests();

                //     global_worldview.iteration += 1;

                //     //Inform all slaves about new orders
                //     global_worldview.name = local_worldview.name.clone();
                //     node.to_slaves_channel().send(global_worldview.clone()).unwrap();
                // }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                match elevator_event {
                    ElevatorEvent::FloorServiced(floor, direction) => {
                        // TODO: Do this conversion somewhere else
                        let direction = match direction {
                            ElevatorDirection::Up => Some(HallRequestDirection::Up),
                            ElevatorDirection::Down => Some(HallRequestDirection::Down),
                            ElevatorDirection::Stopped => None,
                        };

                        if let Some(direction) = direction {
                            node.to_master_channel().send(Message::ClearHallRequest(floor, direction, 0)).unwrap();
                        }

                        node.to_master_channel().send(Message::ClearCabRequest(floor, name.clone(), 0)).unwrap();
                    },
                    ElevatorEvent::StateUpdated(state) => {
        
                        node.to_master_channel().send(Message::ElevatorState(name.clone(), state)).unwrap();
                    },
                }

            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();
                let floor = call_button.floor as usize;

                let new_request_message = match call_button.call {
                    HALL_UP => Message::NewHallRequest(floor, HallRequestDirection::Up),
                    HALL_DOWN => Message::NewHallRequest(floor, HallRequestDirection::Down),
                    CAB => Message::NewCabRequest(floor, name.clone()),
                    invalid_call => {
                        warn!("Received invalid call button: {invalid_call}");
                        continue;
                    },
                };

                node.to_master_channel().send(new_request_message).unwrap();
            },
        }

        // if let Err(e) = save_state_to_file(&local_worldview, "backup.json") {
        //     error!("Could not save backup file: {e}");
        // }
    }
}
