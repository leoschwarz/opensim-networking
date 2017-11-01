# Aim
This project's aim is to implement an efficient networking client for the [OpenSimulator](http://opensimulator.org/wiki/Main_Page) network protocol in Rust.
The goal is to provide a client of high enough quality to implement a new viewer on top of it with matches or surpasses the performance of current viewers.

## Status
As the documentation of the protocol is rather sparse and this library is still not that far, consider this an early work in progress.
There are multiple coexisting "protocols" so following is a list of them and the respective status of their implementation in this library.

**Implemented**:

- UDP messages: Handling of acks works fine. More debugging utilities will have to be added because for viewer development it will most likely be needed.
- Login protocol: Will need some more refinement and better error handling, but it's enough for testing purposes.

**To be implemented soon**:

- Texture download
- Region download

**Not on the current worklist**:

- Sound
- Voice
- Inventory

## Protocol
The main goal of this library is to stay compatible with current versions of OpenSimulator. Since Second Life is diverting from their protocol this library will most likely stop being usable with
their servers (if it isn't already the case).

I'm in the progress of collecting as much documentation on the protocol as possible, to write a good and correct client for it. Many pieces of information are found across
the internet and in various sources, so I'm collecting my own conclusions on the protocol in the repo [opensim-protocol](https://github.com/leoschwarz/opensim-protocol).
Ideally it should be an exact specification of the network protocol implemented by this client.

# Architecture
TODO: Actually this section might not always be that up to date and interesting, most likely it will make more sense to put such information into the relevant sub crates and only
      explain the motivation behind the API actually used by viewers.

## UDP messages
For the UDP messages handling, a code generator written in Ruby is used. The relevant code can be found in the subcrate opensim_messages where you also find
the generated code. The code generator is only needed when updating some of its input files, otherwise pure Rust code can be compiled.

While the generated code bloats the binary size, it's actually one of the easiest ways to implement type safe handling of message data and should allow for
fair performance, up to the point where one should not worry about it unless some performance benchmark indicates pathologically bad performance.
