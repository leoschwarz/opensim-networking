# Aim
This project's aim is to implement an efficient networking client for the [OpenSimulator](http://opensimulator.org/wiki/Main_Page) network protocol in Rust.
Hopefully in the future it will allow for the implementation of NPCs and potentially a viewer written in Rust from scratch.

## Status
Most of the UDP interaction should work by now, and the basic login with XML-RPC should also work.
Usage of this crate is not very ergonomic however, it performs the ack logic, but there are many
messages to be sent and received everywhere.

## TODO
There are some big TODOs:

- Where to get resources from? (I heard there is some new HTTP protocol for that.)
- How is mesh data encoded? We will have to implement our own reader, so it can be used in a potential client not using any of the original Linden viewer code.
- For the sake of it make sure that we are reading quaternions correctly, otherwise funny stuff is going to happen, which will be annoying to debug.
- Generally improve logging further, viewer development will probably need a couple of these inspections where we have to take apart how the protocol works concretely.

## Standard
We are tracking the protocol as used by the OpenSimulator project. Should there be future changes, we will follow their lead. Compatibility with Second Life is desirable but not a goal of the project.

# Architecture
## Generator
As there are many messages and we want to provide a type safe and efficient way to handle these, a bindings generator was written in Ruby.
You can find more information on running it in the `generate/` subfolder's README.


