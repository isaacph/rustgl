# rustgl

Experimental League of Legends clone written in Rust.

Main goals:
* Pure functional gameplay update step
* Perfect network synchronization (with good network conditions)
* Low latency in handling user input

Current features (implemented with the above goals in mind):
* Single-threaded multiplayer server that runs on both UDP and TCP at the same time
    * The server listens on two ports at once, one for each protocol
    * Required or important information is sent through TCP for reliability
    * Short-term and fast-paced information is sent through UDP for speed
    * Old UDP packets are discarded
    * TCP socket is the source of truth for whether you are connected
* Simple player identification system
* Auto attack command closely resembling League of Legends'
* All user actions will cause changes to the world within 2 game ticks of reaching the server
* Simple graphics rendering with OpenGL 3.3

Short-term goals:
* Reduce number of game ticks required to execute a player action (2 -> 1) (requires restructuring update/reduce into more steps)
* Numerically measure how well the synchronization performs under various types of network conditions (currently tested on only with late packets)
* Experiment with multithreading
    * Game update step is already written functionally, allowing for safe multithreading
    * Determine relationship between update function performance, number of units/players, and number of threads
* Create core gameplay loop
    * Assign players to teams, don't allow team killing
    * Create "nexuses" that end the game if destroyed
    * Add respawning
* Add minion AI
* Add collision with terrain and other units
* Add more abilities and "champions"
