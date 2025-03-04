use crossbeam_channel as cbc;
use crossbeam_channel::select;
use driver_rust::elevio::elev::{Elevator, CAB, HALL_DOWN, HALL_UP};
use log::{debug, error, info};
use std::collections::HashSet;
use std::net::SocketAddrV4;

use crate::backup::{load_state_from_file, save_state_to_file};
use crate::elevator::{
    controller::{ElevatorEvent, Behaviour},
    inputs,
    lights::set_call_lights,
};
use crate::network::{advertiser::Advertiser, Client, Host};
use crate::requests::requests::{Direction, Requests};
use crate::worldview::{ElevatorState, HallRequestState, Worldview};

const MASTER_ADVERTISMENT_IP: [u8; 4] = [239, 0, 0, 52];
const MASTER_ADVERTISMENT_PORT: u16 = 52052;

/// Starter TCP-server for Master og fordeler innkommende bestillinger
pub fn start_master_server() {
    // Load state from backup if available
    let mut master_system_state = match load_state_from_file("backup.json") {
        Ok(states) => {
            info!("Loaded backup.");
            states
        }
        Err(_) => {
            info!("No backup found.");
            Worldview::new(String::from("Master"))
        }
    };

    let host = Host::<Worldview>::new_tcp_host(0).unwrap();
    info!("Master lytter på port: {}", host.port());

    // Start å informere slaver om at master eksisterer
    let advertiser = Advertiser::new(
        host.port(),
        MASTER_ADVERTISMENT_IP,
        MASTER_ADVERTISMENT_PORT,
    )
    .unwrap();
    advertiser.start_advertising();

    let mut slave_addresses: HashSet<SocketAddrV4> = HashSet::new();

    loop {
        select! {
            recv(host.receive_channel()) -> message => {
                let (address, recieved_elevator_states) = message.unwrap();
                slave_addresses.insert(address);

                info!("Master mottok melding fra \"{}\":\n{}", &recieved_elevator_states.name, recieved_elevator_states);

                // Legg til nye heiser
                for elevator_state in recieved_elevator_states.elevators.values() {
                    master_system_state.elevators.insert(recieved_elevator_states.name.clone(), elevator_state.clone());
                }

                if recieved_elevator_states.iteration - master_system_state.iteration == 1 {
                     // Ta imot nye og slett fullførte bestillinger
                    for (floor, received_request) in recieved_elevator_states.hall_requests.iter().enumerate() {
                        let master_request = master_system_state.hall_requests[floor].clone();

                        match (&received_request.up, &master_request.up) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => master_system_state.assign_request(floor as u8, Direction::Up),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_system_state.hall_requests[floor].up = HallRequestState::Inactive,
                            _ => {},
                        }

                        match (&received_request.down, &master_request.down) {
                            (HallRequestState::Requested, HallRequestState::Inactive) => master_system_state.assign_request(floor as u8, Direction::Down),
                            (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_system_state.hall_requests[floor].down = HallRequestState::Inactive,
                            _ => {},
                        }
                    }
                }

                master_system_state.iteration += 1;

                // Informere alle slaver om nye bestillinger
                for slave_address in &slave_addresses {
                    host.send_channel().send((*slave_address, master_system_state.to_owned())).unwrap();
                }
            }
        }

        if let Err(e) = save_state_to_file(&master_system_state, "backup.json") {
            error!("klarte ikke lagre backup: {e}");
        }
    }
}

pub fn send_state_to_maser(
    client: &Client<Worldview>,
    mut system_state: Worldview,
    local_elevator_state: ElevatorState,
) {
    system_state.set_local_elevator_state(local_elevator_state);
    system_state.iteration += 1;
    client.send_channel().send(system_state).unwrap();
}

/// Kobler opp til en master tjener. Sender bestillingsforespørsler og utfører mottatte bestillinger.
pub fn start_slave_client(
    name: Option<String>,
    elevio_elevator: &Elevator,
    elevator_command_tx: cbc::Sender<Requests>,
    elevator_event_rx: cbc::Receiver<ElevatorEvent>,
) {
    let input_channels = inputs::InputChannels::new(elevio_elevator);

    let advertiser = Advertiser::new(0, MASTER_ADVERTISMENT_IP, MASTER_ADVERTISMENT_PORT).unwrap();

    info!("Leter etter en master...");
    let (master_address, master_port) = advertiser.receive_channel().recv().unwrap();
    info!("Fant en master: {master_address} {master_port}");

    let client: Client<Worldview> =
        Client::new_tcp_client(master_address.ip().octets(), master_port).unwrap();
    info!("Koblet til master!");

    // Bruk et tilfeldig dyr som id dersom navn ikke er spesifisert:)
    let name = name.unwrap_or(petname::petname(1, "").unwrap());

    let mut local_elevator_state = ElevatorState {
        state: Behaviour::Idle,
        cab_requests: [false; 4],
        direction: Direction::Up,
        floor: 0,
    };

    let mut system_state = Worldview::new(name);

    loop {
        cbc::select! {
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                // Oppdater tilstand til lokal heis
                local_elevator_state.floor = elevator_event.floor;
                local_elevator_state.direction = elevator_event.direction;
                local_elevator_state.state = elevator_event.state;

                // Marker ordre i etasje som fullførte
                local_elevator_state.cab_requests[elevator_event.floor as usize] = false;

                if elevator_event.direction != Direction::Down {
                    debug!("Cleared up.");
                    system_state.hall_requests[elevator_event.floor as usize].up = HallRequestState::Inactive;
                }
                if elevator_event.direction != Direction::Up {
                    debug!("Cleared down.");
                    system_state.hall_requests[elevator_event.floor as usize].down = HallRequestState::Inactive;
                }

                // Send den oppdaterte ordrelisten til heiskontrolleren
                let requests = system_state.requests_for_local_elevator();
                elevator_command_tx.send(requests).unwrap();

                // Informer master om den nye tilstanden
                send_state_to_maser(&client, system_state.clone(), local_elevator_state.clone());
            },
            recv(input_channels.call_button_rx) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;
                let hall_request = &mut system_state.hall_requests[floor];

                // Legg inn bestilling på etasje
                match call_button.call {
                    HALL_UP if hall_request.up == HallRequestState::Inactive => hall_request.up = HallRequestState::Requested,
                    HALL_DOWN if hall_request.down == HallRequestState::Inactive => hall_request.down = HallRequestState::Requested,
                    CAB => local_elevator_state.cab_requests[floor] = true,
                    _ => {},
                }

                // Informer master om den nye tilstanden
                send_state_to_maser(&client, system_state.clone(), local_elevator_state.clone());
                system_state.set_local_elevator_state(local_elevator_state.clone());
                client.send_channel().send(system_state.clone()).unwrap();

            },
            recv(client.receive_channel()) -> message => {
                let (_, master_state) = message.unwrap();

                system_state.sync_with_master(master_state);
                system_state.set_local_elevator_state(local_elevator_state.clone());

                info!("Received state from master:\n{system_state}");

                // Send den nye bestillingslista til heiskontrolleren og lyskontrolleren
                let requests = system_state.requests_for_local_elevator();
                set_call_lights(&elevio_elevator, &requests);
                elevator_command_tx.send(requests).unwrap();

            },
        }

        if let Err(e) = save_state_to_file(&system_state, "backup.json") {
            error!("Klarte ikke å lagre backup: {e}");
        }
    }
}
