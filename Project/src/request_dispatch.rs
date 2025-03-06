use crossbeam_channel::{self as cbc, tick};
use crossbeam_channel::{select, Sender};
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::backup::save_state_to_file;
use crate::elevator::inputs::create_call_button_channel;
use crate::elevator::{controller::ElevatorEvent, lights::set_call_lights};
use crate::network::node::Node;
use crate::requests::requests::{Direction, Requests};
use crate::worldview::{HallRequestState, Worldview};

pub fn send_state_to_maser(to_master: &Sender<Worldview>, mut worldview: Worldview) {
    let local_elevator = worldview.local_elevator_state();
    local_elevator.timestamp_last_event = SystemTime::now();
    local_elevator.active = true;
    worldview.iteration += 1;
    to_master.send(worldview).unwrap();
}

/// Starter TCP-server for Master og fordeler innkommende bestillinger
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

                // Send den nye bestillingslista til heiskontrolleren og lyskontrolleren
                let requests = worldview.requests_for_local_elevator();

                set_call_lights(&elevio_driver, &requests);
                elevator_command_tx.send(requests).unwrap();
            },
            recv(node.from_slave_channel()) -> message => {
                let slave_worldview = message.unwrap();
                let slave_name = &slave_worldview.name;

                info!("Master mottok melding fra \"{slave_name}\":\n{slave_worldview}");

                // Dersom vi har mottatt en melding fra en deaktivert slave kan vi
                // anta at den er tilbake og aktivere den igjen
                if let Some(elevator) = worldview.elevators.get(slave_name) {
                    if !elevator.active {
                        info!("Aktiverer \"{}\" :)", slave_name);
                    }
                } else {
                    info!("Ny slave tilkoblet \"{}\"", slave_name);
                }

                let mut slave_elevator_state = slave_worldview.elevators[slave_name].clone();
                slave_elevator_state.timestamp_last_event = SystemTime::now();
                worldview.elevators.insert(slave_name.clone(), slave_elevator_state);

                if slave_worldview.iteration - worldview.iteration == 1 {
                    // Ta imot nye og slett fullførte bestillinger
                    for (floor, received_request) in slave_worldview.hall_requests.iter().enumerate() {
                        let master_request = worldview.hall_requests[floor].clone();

                        match (&received_request.up, &master_request.up) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => worldview.add_request(floor as u8, Direction::Up),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => worldview.hall_requests[floor].up = HallRequestState::Inactive,
                            _ => {},
                        }

                        match (&received_request.down, &master_request.down) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => worldview.add_request(floor as u8, Direction::Down),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => worldview.hall_requests[floor].down = HallRequestState::Inactive,
                            _ => {},
                        }
                    }

                    worldview.assign_requests();
                } else {
                    warn!("Mottok ugyldig verdenssyn.")
                }

                worldview.iteration += 1;

                node.to_slaves_channel().send(worldview.clone()).unwrap();
            },
            // Start å informere slaver om at master eksisterer
            recv(ticker) -> _ => {
                // Hent nåværende tidspunkt
                let timestamp_start_master_server = SystemTime::now();
                let mut changed = false;

                let requests_map: HashMap<_, _> = worldview
                    .elevators
                    .keys()
                    .map(|name| (name.clone(), worldview.requests_for_elevator(name)))
                    .collect();

                // Gå gjennom alle heiser og hent timestampen for tildelte forespørsler
                for (name, elevator) in &mut worldview.elevators {
                    if let Ok(duration) = timestamp_start_master_server.duration_since(elevator.timestamp_last_event) {
                        let has_orders = requests_map[name].unwrap().iter().any(|v| v.hall_up || v.hall_down || v.cab);

                        if elevator.active && has_orders && duration > Duration::from_secs(5) {
                            info!("Deaktiverer {name} :(");
                            elevator.active = false;
                            changed = true;
                        }
                    }
                }

                if changed {
                    worldview.assign_requests();

                    worldview.iteration += 1;

                    // Informere alle slaver om nye bestillinger
                    node.to_slaves_channel().send(worldview.clone()).unwrap();
                }
            },
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                let local_elevator_state = worldview.local_elevator_state();

                // Oppdater tilstand til lokal heis
                local_elevator_state.floor = elevator_event.floor;
                local_elevator_state.direction = elevator_event.direction;
                local_elevator_state.behaviour = elevator_event.state;

                // Marker ordre i etasje som fullførte
                local_elevator_state.cab_requests[elevator_event.floor as usize] = false;

                if elevator_event.direction != Direction::Down {
                    debug!("Cleared up.");
                    worldview.hall_requests[elevator_event.floor as usize].up = HallRequestState::Inactive;
                }
                if elevator_event.direction != Direction::Up {
                    debug!("Cleared down.");
                    worldview.hall_requests[elevator_event.floor as usize].down = HallRequestState::Inactive;
                }

                // Send den oppdaterte ordrelisten til heiskontrolleren
                let requests = worldview.requests_for_local_elevator();
                elevator_command_tx.send(requests).unwrap();

                // Informer master om den nye tilstanden
                send_state_to_maser(node.to_master_channel(), worldview.clone());
            },
            recv(call_button_channel) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;
                let hall_request = &mut worldview.hall_requests[floor];

                // Legg inn bestilling på etasje
                match call_button.call {
                    HALL_UP if hall_request.up == HallRequestState::Inactive => hall_request.up = HallRequestState::Requested,
                    HALL_DOWN if hall_request.down == HallRequestState::Inactive => hall_request.down = HallRequestState::Requested,
                    CAB => worldview.local_elevator_state().cab_requests[floor] = true,
                    _ => {},
                }

                // Informer master om den nye tilstanden
                send_state_to_maser(node.to_master_channel(), worldview.clone());
            },
        }

        if let Err(e) = save_state_to_file(&worldview, "backup.json") {
            error!("Klarte ikke å lagre backup fil: {e}");
        }
    }
}
