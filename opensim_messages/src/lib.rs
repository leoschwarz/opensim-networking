extern crate byteorder;
extern crate uuid;
extern crate opensim_types;

use opensim_types::*;

/// Contains all available messages.
pub mod all;

// TODO rethink this later
pub use all::*;
