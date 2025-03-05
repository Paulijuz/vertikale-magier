# Welcome
Welcome to group 52's amazing elevator "vertikale-magier"!

This is the infamours elevator project for the coure [TTK4145 Real-time Programming](https://www.ntnu.edu/studies/courses/TTK4145) at NTNU. The task is to reliably control three elevators in a distributed system.

# Outline
The project consits of the following modules.
- `network` - Contains everything network related. (duh)
    - `client` - Out "black-box" wrapper for sockets (both TCP and UDP). It exposes channels for sending and receiving data. The data is automatically serialized and deserialized using `serde` and `serde_json`.
    - `host` - TCP server which accepts connections. It uses the above `client` to handle the incomming connections.
    - `advertiser` - A utility for sending out messages periodically over UDP. It uses the `client` module for low level sockets.
    - `node` - TODO
- `elevator` -  Contains hardware related modules.
    - `intputs` - Wraps the elevio poll functions in channels for ease of use.
    - `ligts` - Simple functions for setting lights.
    - `controller` - The FSM for controlling the elevator. Receives requests as inputs and sends elevator state as output.
- `requests` - Contains request related functions and types.
    - `requests` - Contains structs, types and functions for storing and mutating requests.
    - `assigner` - Our wrapper for ["hall_request_assigner"](https://github.com/TTK4145/Project-resources/tree/master/cost_fns/hall_request_assigner).
- `request_dispatcher` - In essence the place where `network` meets `requests`. It takes in button presses, assigns requessts and distributes them among the elevators. 
- `worldview` - Structs and corresponding functions to store and mutate a worldview of the system.
- `main` - Is this counted as a module? In any case, it simply starts up the elevato
