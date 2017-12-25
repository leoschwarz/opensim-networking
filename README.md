# Goal
This project's aim is to implement an efficient networking client for the [OpenSimulator](http://opensimulator.org/wiki/Main_Page) network protocol in Rust.
The goal is to provide a client of high enough quality to implement a new viewer on top of it with matches or surpasses the performance of current viewers.

## Compiler version
You need nightly Rust for this, as I decided that using `futures-await` is going to save more than enough development time to make this worth it already.

## Status
As the documentation of the protocol is rather sparse and this library is still not that far, consider this an early work in progress.
There are multiple coexisting "protocols" so following is a list of them and the respective status of their implementation in this library.

**Implemented**:

- UDP messages: Handling of acks works fine. More debugging utilities will have to be added because for viewer development it will most likely be needed.
- Login protocol: Will need some more refinement and better error handling, but it's enough for testing purposes.

**Currently being worked on**:

- Texture download
- Region download

**Soon to be worked on**:
- Prims
- Mesh data

**Backlog**:

- Sound
- Voice
- Inventory

## Protocol
The main goal of this library is to stay compatible with current versions of OpenSimulator. Since Second Life is diverting from their protocol this library will most likely stop being usable with
their servers (if it isn't already the case).

I'm in the progress of collecting as much documentation on the protocol as possible, to write a good and correct client for it. Many pieces of information are found across
the internet and in various sources, so I'm collecting my own conclusions on the protocol in the repo [opensim-protocol](https://github.com/leoschwarz/opensim-protocol).
Ideally it should be an exact specification of the network protocol implemented by this client.
