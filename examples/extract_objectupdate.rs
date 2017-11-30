extern crate llsd;
use std::io::BufReader;

fn main() {
    // TODO: This is not llsd but it's own encoding -> see protocol repo
    let raw_data = vec![
        138, 231, 80, 74, 0, 1, 105, 69, 167, 61, 0, 0, 0, 0, 10, 37, 127, 63, 181, 100, 177, 65,
        71, 93, 41, 67, 200, 44, 255, 66, 200, 50, 26, 65, 217, 125, 254, 127, 138, 129, 255, 127,
        255, 127, 255, 127, 255, 127, 255, 127, 255, 127, 255, 255, 255, 127, 255, 127, 255, 127,
    ];

    let mut reader = BufReader::new(&raw_data[..]);

    let data = llsd::binary::read_value(&mut reader).unwrap();
    println!("data: {:?}", data);
}
