# Peer reviews
Scores are given on a scale from 5 to 9.

## Reviewer 1
**Score: 8**
- Comments and debug prints are some places in Norwegian, which is red flag for maintenance.
- Nice and linear structure of the code: every module points to request_dispatch which talks to the node. Nice module_diagram which helps.
- As far as I can tell from node.rs and advertiser.rs, the selection for who is master is too vulnerable. If a master dies, there is a real chance that all slaves turn master, and kills themselves simultaneously and orders are lost.
- There is no timer that promotes an elevator from slave to master if the master stops transmitting (dies); but only if the master transmits an erroroneous message. See line 145+ in node.rs.
- Clever definition of the ElevatorController in elevator->controller.rs. Good structure and readability to write ElevatorController as an object with methodes including the "next_direction" (FSM).
- I think Worldview is a bad name for what worldview.rs does. A worldview sounds like an data structure, and should not have these methods, maybe except for "local_elevator_state" and "sync_with_master". All the methods regarding requests and running the hall_request_assigner should be packed elsewhere.
- They have refactored the C-code-FSM from single-elevator from github, in a really good way. Turning away from storing button_Request as uint8, and rather treat them as enum where all the possible states are easy readable.
- It is not apparent what the buffer in client.rs sends via TCP. If worldview are sendt via TCP the system will be slow, and not scale well.
- The function requests_for_local_elevator seems redundant as it practically only serves to obfuscate the call to requests_for_elevator? Maybe there is some other intent behind this, but in that case it's not clearly communicated.

## Reviewer 2
**Score: 9**
- you are using clap for command line arguments, but it isn't fully integrated into the code.
- why is there a command ./hall_request_assigner. how is it created
- SendableType is a terrible name.
- why are you only reseting hartbeat on hartbeat message. why not any message from a spesific client?
- advertiser is a FSM which is controlled by crossbeam channels???
- seems like you have put allot of functionality into "ElevatorController". more or less all logic of the lift is inside it.
- the type name Request is a bit bad. especialy since there is a Requests
- does the type worldview have duplicate/overlapping data???
- run_dispatcher() is huge.
- you may get a bug if you are to slow at reading internet messages. serde_json needs some care if there are multiple json objects in the buffer.
- you are very strict with making your types private.
- if you are going to have set and get functions. indicate that it is what they are. Node::from_slave_channel() is ok, but not great.
- the function is getting the cannel for comunication from(receiving) slave (get_slave_channel_recieving??).
- you have done a good job of importing modules (use crate:: VS. use super::). only "mistake" is in node.rs
- you have allot of code obfuscated in your objects. make sure side effects are communicated to the user.
- i am a bit scared that Client can spawn threads.
- your figure indicates that most of the boxes are dependent on request_dispatch.
- The main function is well-structured, setting up logging, elevator driver, and core modules cleanly.
- The system structure appears well-organized, but the interaction between the FSM (controller.rs), networking, and request handling could be more clearly defined.
- The system uses both UDP and TCP, with TCP handling direct communication and UDP for status updates, but their integration with other modules is not immediately clear.
- Threads are created for handling elevator control and request dispatch, but their responsibilities could be documented more explicitly.
- The system follows a master-slave architecture, but additional documentation on leader election and failure handling would improve clarity.
- Request handling is spread across multiple modules, which makes it unclear where requests are finalized, but this is already mentioned in the README as something being worked on.
- conclution: the code seems very good and well organized. on the other hand the amount of code makes it hard to digest. the names are for the most part ok, but some of them dont give enough context of what they are.

## Reviewer 3
**Score: 9**
- The entry point code is well crafted. Just reading through main.rs alone paints a very clear picure of the things that are happening at the top-level. There are almost no comments in main.rs, but this is not even a problem because of how readable and clean the code is. The project also has a very descriptive and nice readme file that explains the high-level design of the project, and is very helpful to getting to know the codebase. However, since main.rs should be able to be used as a black box (take command line arguments and just work), i feel like there should be a short section early on, either in the readme or a comment in main.rs itself, that breifly describes how the program should be used. What arguments does it take? What does it do? Reading through the args struct gives information about the program args, but it doesn't say what the program does, and i should not have to start reading code to know what arguments to pass to a program.
- From a outside point of view, the choice of module Structure and naming (the interface in general) is good, and gives a clear indication of the functionality of the module. This makes it a lot easier to get up to speed on the codebase, and provides for seamless integration of potential new developers/collaborators.
- From a inside point of view the code has good usage of "modular" functions (smaller functions that call other smaller functions). The code is compact and readable, and despite this the number of small functions is held reasonably low. It could however benefit from a bit more modularity here and there, especially wrapping the handling of "things" inside handler functions. This would make the high-level thought behind the code easier to follow. Having the concrete handling of specific cases in a high-level of abstraction function ruins the readability of the code. If i want to know how a certain case is handled, i should just go to that function. All i need to know in the high-level function is that all cases are probed for and handled accordingly. 
- The details of the code are good. Names are very descriptive, and makes for easy reading. The commenting could be better in the lower-level code. Since the naming is so good, commenting is really not neccessary in the high-level of abstraction code. However in the lower levels, comments can make it easier to follow along, without always having to infer the purpose of some line of code.
- My gut feeling is that this group has handled the project well. The design seems thought through, and the code is well crafted. There could be made some improvements here and there, but this is just nitpicking and well within the capabilities of the group. The proposed improvements just consists of improving the separation of higher and lower level of abstraction code, and adding some comments in lower level code. This is not too hard, and the group could easily manage this. 

## Reviewer 4
**Score: 9**
- Top-level entry points: The main function initiates a lot of key components in the system, and it shows how these different parts of the system are connected, It does not include any unnecessary information. Its missing comments clarifying the intention of each function; however, the naming is clear and this does reduce the effect of the lack of comments. This main gets an 8/10.
- Module-architecture “outside”: Your modules are logically separated and seems to execute task effectively. I like that you make a module for the requests themselves, this is good separation, I also like your use of the mod.rs and module separation I general, where you have a consistent project structure. The outside architecture gets an 9/10.
- Structure of modules “inside”: Threads are spawed with clear intention and are managed I quite simple ways, this is good. I’d say the functions in the modules are as pure as to be expected and they are placed in the logical modules. You use the function unwrap() quite frequently can lead to unexpected panics or errors, so this is something you’d might want to think about. I give this a 7/10.
- Testability: The biggest fault here could be that you do not include logging in your code, which would make error detection and understanding while running much easier. But you good separation otherwise makes each individual functionality in the code testable, or at least most functionalities are testable, this is very good. I Give this 7/10.
- Interaction between modules: The system has a well-structured, decoupled design where each module has a distinct responsibility, the modules interact with channels and only seem so share useful information, and they to if efficiently. I give this 9/10.
- Commenting/useful naming: You commenting is lacking, it would be very helpful for an outsider to be able to understand the intention of each part of the code without having to read the code itself, however your naming is very good, and makes your code readable and intentions clear. I give this a 6/10. 
- Gut feeling: You have understood the problems in this project and are far along in the process of solving them, and I think you will be able to finish before the deadline if you continue working like you have done so far. Well done.

