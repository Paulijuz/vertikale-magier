# Welcome
Welcome to group 52's amazing elevator "vertikale-magier"!

This is the infamours elevator project for the course [TTK4145 Real-time Programming](https://www.ntnu.edu/studies/courses/TTK4145) at NTNU. The task is to reliably control three elevators in a distributed system.

# Outline
The project consits of the following modules.
- `network` - Contains everything network related.
    - `client` - Our wrapper for sockets (both TCP and UDP). It exposes channels for sending and receiving data. The data is automatically serialized and deserialized using `serde` and `serde_json`.
    - `host` - TCP server which accepts connections. It uses the above `client` to handle the incomming connections.
    - `advertiser` - A utility for sending out messages periodically over UDP. It uses the `client` module for low level sockets.
    - `node` - A
- `elevator` -  Contains hardware related modules.
    - `intputs` - Wraps the elevio poll functions in channels for ease of use.
    - `ligts` - Simple functions for setting lights.
    - `controller` - The FSM for controlling the elevator. Receives requests as inputs and sends elevator state as output.
- `requests` - Contains request related functions and types.
    - `requests` - Contains structs, types and functions for storing and mutating requests.
    - `assigner` - Our wrapper for ["hall_request_assigner"](https://github.com/TTK4145/Project-resources/tree/master/cost_fns/hall_request_assigner).
- `request_dispatcher` - In essence the place where `network` meets `requests`. It takes in button presses, assigns requessts and distributes them among the elevators. 
- `worldview` - Structs and corresponding functions to store and mutate a worldview of the system.
- `timer` - Starts a timer for a specified duration, after a signal is sent through a channel to indicate timeout.
- `backup` - function for loading and saving `worldview` into a file.

A module diagram can be found at the root of the project.

# Note for Peer Review
This project is still work in progress (as with many of the other groups), so there are quite a few things that we have though about, but have not come around to doing yet. Specificaly, when it comes to code quality we want to:
- Organize the way requests are handled. Currently, this is spread throughout the project, but ideally a lot of common functionality and structures should be located in one place. This is what the "requests" module should be, but it's a bit lacking at the moment.
- Restrucutre the elevator controller struct. This struct currently holds the elevator state, the requests, a timer, and the driver. This results in its implementaiton functons being quite messy as the they do a bit of everything.
- Try to minimize the things happening in request_dispatcher. This is maybe our most central, but also most "hand-wavy", module as it is where all the different systems meet. It'd be nice to split it up futher to make the modules have clearer tasks.