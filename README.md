# Aim
This project's aim is to implement an efficient networking client for the [OpenSimulator](http://opensimulator.org/wiki/Main_Page) network protocol in Rust.
Hopefully in the future it will allow for the implementation of NPCs and potentially a viewer written in Rust from scratch.

## Status
Note that as of now the project is in an early stage of development. I'm not very experienced with many parts of the library, especially async IO. Hence many parts of the library are yet going to see a lot of restructuring and there is no first release in sight yet.

## Standard
We are tracking the protocol as used by the OpenSimulator project. Should there be future changes, we will follow their lead. Compatibility with Second Life is desirable but not a goal of the project.

# Architecture
## Generator
As there are many messages and we want to provide a type safe and efficient way to handle these, a bindings generator was written in Ruby.
You can find more information on running it in the `generate/` subfolder's README.


