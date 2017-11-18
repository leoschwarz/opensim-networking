# LLSD Rust

Linden Lab Structured Data (LLSD) describes a data interchange format, widely used in the LL/OpenSim protocol.

## Format documentation

The main point of documentation of the format can be found on the [Second Life Wiki page on LLSD](http://wiki.secondlife.com/wiki/LLSD).

## Implementation notes

- XML: Binary encoding only BASE64, decoding only BASE16 and BASE64 but no BASE85.
