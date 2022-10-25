# rustgl

Experimental League of Legends clone written in Rust.

## Preview
(Apologies for poor GIF quality, first time trying to do it)

Autoconnect to a running server, then login to generate a unit:

![rustglclip1](https://user-images.githubusercontent.com/37312022/197681602-02bbce7d-e733-46c4-8ba7-4e90480fd809.gif)

Split screen of two clients, demonstrating movement:

![rustglclip2](https://user-images.githubusercontent.com/37312022/197680343-281f7e47-9100-47f8-a799-2a762b98f69c.GIF)

Split screen demo of auto attack ability, mimicking League's auto attack:

![rustglclip3-1600](https://user-images.githubusercontent.com/37312022/197684369-08ae2ac5-bd21-4b9c-ad70-f12fd0623680.gif)

Split screen demo of auto attack follow, and "flash" ability (mimicking Ezreal E from League):

![rustglclip4-1600](https://user-images.githubusercontent.com/37312022/197685188-0fdf88a9-ac45-46ec-8f09-6845be509d1f.gif)

Note the main performance bottleneck (reason why most clips aren't 240 fps) is the CPU-side draw list construction step (GPU side is very fast). Basically I'm recreating and re-sorting drawing data from scratch every frame, which turns out to be slow enough to make my new high-end CPU unable to stay at 240 FPS. Will fix this in the future with the planned WGPU port.

Also note that if there is any difference between a client and the server whatsoever, the chatbox of that client will start spamming messages describing the difference. Since the chat is silent, in the given demos you can see byte-perfect synchronization (this has also been tested over long distances and works as long as network connection is mostly stable).

## Description
Main goals:
* Pure functional and deterministic gameplay update step
* Perfect network synchronization (with good network conditions)
* Low latency in handling user input

Current features (implemented with the above goals in mind):
* Single-threaded multiplayer server that runs on both UDP and TCP at the same time
    * The server listens on two ports at once, one for each protocol
    * Required or important information is sent through TCP for reliability
    * Short-term and fast-paced information is sent through UDP for speed
    * Old UDP packets are discarded
    * TCP socket is the source of truth for whether you are connected
* Client-side rollback netcode for catching late packets
    * When a client receives a packet, it files the packet into the correct tick ID
    * The client keeps a version of the world multiple ticks backwards so that late packets can be merged with the current world by rerunning previous frames/ticks
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
