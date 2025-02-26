use crate::elevator_controller::{Direction, ElevatorEvent, ElevatorRequests, Request};
use crate::elevator_controller::{State, NUMBER_OF_FLOORS};
use crate::hall_request_assigner as hra;
use crate::inputs;
use crate::network::advertiser::Advertiser;
use crate::network::socket::{Client, Host};
use crate::light_sync::sync_call_lights;
use crate::backup::{load_state_from_file, save_state_to_file};

use core::fmt;
use crossbeam_channel as cbc;
use crossbeam_channel::select;
use driver_rust::elevio;
use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::array;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddrV4;



#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SingleElevatorState {
    pub name: String,
    pub direction: Direction,
    pub state: State,
    pub floor: u8, // TOOD: Denne typen kan vel egentlig være usize?
    pub cab_requests: [bool; NUMBER_OF_FLOORS],
}

impl From<&SingleElevatorState> for hra::State {
    fn from(single_elevator_state: &SingleElevatorState) -> Self {
        hra::State {
            behaviour: match single_elevator_state.state {
                State::DoorOpen => hra::Behaviour::DoorOpen,
                State::Moving => hra::Behaviour::Moving,
                _ => hra::Behaviour::Idle,
            },
            floor: single_elevator_state.floor,
            direction: match single_elevator_state.direction {
                Direction::Down => hra::Direction::Down,
                Direction::Stopped => hra::Direction::Stop,
                Direction::Up => hra::Direction::Up,
            },
            cab_requests: single_elevator_state.cab_requests,
        }
    }
}

impl fmt::Display for SingleElevatorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Navn: {}\nTilstand: {:?}\nRetning: {:?}\nEtasje: {}\nInterne bestillinger: {:?}",
            self.name,
            self.state,
            self.direction,
            self.floor + 1,
            self.cab_requests
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] //gir enumen den tilgangen (må stå over hver enum)
enum ElevatorRole {
    Master,
    Slave,
}
//vet ikke om vi trenger den over^

// MasterState: Holder oversikt over alle heiser og bestillinger

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum HallRequestState {
    Inactive,
    Requested,
    Assigned(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HallRequest {
    up: HallRequestState,
    down: HallRequestState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllElevatorStates {
    pub elevators: HashMap<String, SingleElevatorState>, //Liste over alle aktive heiser
    pub hall_requests: [HallRequest; NUMBER_OF_FLOORS],
}

impl fmt::Display for AllElevatorStates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Heiser:")?;
        for (id, elevator_state) in &self.elevators {
            writeln!(f, "  {id}:")?;

            for line in elevator_state.to_string().lines() {
                writeln!(f, "    {line}")?;
            }
        }

        writeln!(f, "Bestillinger:")?;
        for (mut floor, hall_request) in self.hall_requests.iter().enumerate().rev() {
            floor += 1;

            writeln!(
                f,
                "  Etasje {floor} - Ned: {:?}, Opp: {:?}",
                hall_request.down, hall_request.up
            )?;
        }

        Ok(())
    }
}

//endre til floor variabelen i vec^

//udp for å finne master
impl AllElevatorStates {
    pub fn new() -> Self {
        Self {
            elevators: HashMap::new(),
            hall_requests: array::from_fn(|_| HallRequest {
                up: HallRequestState::Inactive,
                down: HallRequestState::Inactive,
            }),
        }
    }

    // Velger beste heis for en bestilling
    pub fn assign_request(&mut self, floor: u8, direction: Direction) {
        match direction {
            Direction::Up => self.hall_requests[floor as usize].up = HallRequestState::Requested,
            Direction::Down => {
                self.hall_requests[floor as usize].down = HallRequestState::Requested
            }
            _ => panic!("Tried to assign request with invalid direction"),
        }

        let hall_requests = self.hall_requests.clone().map(|request| {
            (
                request.up != HallRequestState::Inactive,
                request.down != HallRequestState::Inactive,
            )
        });
        let states = self
            .elevators
            .iter()
            .map(|(k, v)| (k.to_owned(), v.into()))
            .collect();

        let assignments = hra::run_hall_request_assigner(hra::HallRequestsStates {
            hall_requests,
            states,
        })
        .unwrap();

        for (id, assigned_hall_requests) in assignments.iter() {
            for (floor, (up, down)) in assigned_hall_requests.iter().enumerate() {
                if *up {
                    self.hall_requests[floor].up = HallRequestState::Assigned(id.to_string());
                }

                if *down {
                    self.hall_requests[floor].down = HallRequestState::Assigned(id.to_string());
                }
            }
        }
    }

    pub fn get_requests_for_elevator(&self, name: &String) -> Option<ElevatorRequests> {
        let mut requests = [Request {
            cab: false,
            hall_down: false,
            hall_up: false,
        }; NUMBER_OF_FLOORS];

        for (floor, cab_request) in self.elevators.get(name)?.cab_requests.iter().enumerate() {
            requests[floor].cab = *cab_request;
        }

        for (floor, hall_request) in self.hall_requests.iter().enumerate() {
            requests[floor].hall_up = hall_request.up == HallRequestState::Assigned(name.clone());
            requests[floor].hall_down =
                hall_request.down == HallRequestState::Assigned(name.clone());
        }

        return Some(requests);
    }
}



/// Starter TCP-server for Master og fordeler innkommende bestillinger
pub fn start_master_server() {

    let mut master_elevator_states = AllElevatorStates::new();

    // Load state from backup if available
    if let Ok(state) = load_state_from_file("backup.json") {
        master_elevator_states = state;
        info!("lastet inn tilstandene til hver heis til back-up");
    }

    let master: Host<AllElevatorStates> = Host::new_tcp_host(None);
    info!("Master lytter på port {}", master.port());

    // Start å informere slaver om at master eksisterer
    let advertiser = Advertiser::init(master.port());
    advertiser.start_advertising();

    
    let mut slave_addresses: HashSet<SocketAddrV4> = HashSet::new();
    
    

    loop {
        select! {
            recv(master.receive_channel()) -> message => {
                let (address, recieved_elevator_states) = message.unwrap();
                slave_addresses.insert(address);

                info!("Master mottok melding fra slave:\n{}", recieved_elevator_states);

                // Legg til nye heiser
                for elevator_state in recieved_elevator_states.elevators.values() {
                    master_elevator_states.elevators.insert(elevator_state.name.clone(), elevator_state.clone());
                }

                // Ta imot nye og slett fullførte bestillinger
                for (floor, received_request) in recieved_elevator_states.hall_requests.iter().enumerate() {
                    let master_request = master_elevator_states.hall_requests[floor].clone();

                    match (&received_request.up, &master_request.up) {
                        (HallRequestState::Requested, HallRequestState::Inactive) => master_elevator_states.assign_request(floor as u8, Direction::Up),
                        (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_elevator_states.hall_requests[floor].up = HallRequestState::Inactive,
                        _ => {},
                    }

                    match (&received_request.down, &master_request.down) {
                        (HallRequestState::Requested, HallRequestState::Inactive) => master_elevator_states.assign_request(floor as u8, Direction::Down),
                        (HallRequestState::Inactive, HallRequestState::Assigned(_)) => master_elevator_states.hall_requests[floor].down = HallRequestState::Inactive,
                        _ => {},
                    }
                }

                // Informere alle slaver om nye bestillinger
                for slave_address in &slave_addresses {
                    master.send_channel().send((*slave_address, master_elevator_states.to_owned())).unwrap();
                }
            }
        }
        if let Err(e) = save_state_to_file(&master_elevator_states, "backup.json") {
            error!("klarte ikke lagre tilstanden: {}", e);
            info!("master_elevator_states er lagret i back-upen")
        }
    }
    
}

/// Kobler opp til en master tjener. Sender bestillingsforespørsler og utfører mottatte bestillinger.
pub fn start_slave_client(
    elevio_elevator: &elevio::elev::Elevator,
    elevator_command_tx: cbc::Sender<ElevatorRequests>,
    elevator_event_rx: cbc::Receiver<ElevatorEvent>,
) {
    let rx_channels = inputs::get_input_channels(&elevio_elevator);

    let advertiser = Advertiser::init(0u16);

    info!("Leter etter en master...");
    let (master_address, master_port) = advertiser.receive_channel().recv().unwrap();
    info!("Fant en master: {master_address} {master_port}");

    let slave: Client<AllElevatorStates> =
        Client::new_tcp_client(master_address.ip().octets(), master_port).unwrap();
    info!("Koblet til master!");

    // Bruk et tilfeldig dyr som id :)
    let name = petname::petname(1, "").unwrap();

    let mut local_elevator_state = SingleElevatorState {
        name: name.clone(),
        state: State::Idle,
        cab_requests: [false; 4],
        direction: Direction::Up,
        floor: 0,
    };

    let mut all_elevator_states = AllElevatorStates::new();
    


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
                    all_elevator_states.hall_requests[elevator_event.floor as usize].up = HallRequestState::Inactive;
                }
                if elevator_event.direction != Direction::Up {
                    debug!("Cleared down.");
                    all_elevator_states.hall_requests[elevator_event.floor as usize].down = HallRequestState::Inactive;
                }

                // Send den oppdaterte ordrelisten til heiskontrolleren
                if let Some(requests) = all_elevator_states.get_requests_for_elevator(&name) {
                    elevator_command_tx.send(requests).unwrap();
                }
                // Informer master om den nye tilstanden
                all_elevator_states.elevators.insert(name.clone(), local_elevator_state.clone());
                slave.sender().send(all_elevator_states.clone()).unwrap();
            },
            recv(rx_channels.call_button_rx) -> call_button => {
                let call_button = call_button.unwrap();

                let floor = call_button.floor as usize;
                let hall_request = &mut all_elevator_states.hall_requests[floor];

                // Legg inn bestilling på etasje
                match call_button.call {
                    HALL_UP if hall_request.up   == HallRequestState::Inactive => hall_request.up = HallRequestState::Requested,
                    HALL_DOWN if hall_request.down == HallRequestState::Inactive => hall_request.down = HallRequestState::Requested,
                    CAB => local_elevator_state.cab_requests[floor] = true,
                    _ => {},
                }

                // Informer master om den nye tilstanden
                all_elevator_states.elevators.insert(name.clone(), local_elevator_state.clone());
                slave.sender().send(all_elevator_states.clone()).unwrap();

            },
            recv(slave.receiver()) -> message => {
                let (_, master_state) = message.unwrap();

                all_elevator_states = master_state;
                all_elevator_states.elevators.insert(name.clone(), local_elevator_state.clone());

                let requests = all_elevator_states.get_requests_for_elevator(&name).unwrap();
                sync_call_lights(&elevio_elevator, &requests);


                info!("Received state from master:\n{all_elevator_states}");

                // Send den nye bestillingslista til heiskontrolleren
                if let Some(requests) = all_elevator_states.get_requests_for_elevator(&name) {
                    elevator_command_tx.send(requests).unwrap();
                }
                
            },
        }
        if let Err(e) = save_state_to_file(&all_elevator_states, "backup.json"){
            error!("Klarte ikke sende den nye bestillingslista til heiskontrolleren i back-up: {}", e);
            info!("Sendt den nye bestillingslista til heiskontrolleren i back-up")
        }
    }
}


