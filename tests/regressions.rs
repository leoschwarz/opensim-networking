extern crate opensim_networking;

use opensim_networking::packet::Packet;

#[test]
fn packet_zerocoded1()
{
    let data = include_bytes!("data/packet_zerocoded1.bin");
    Packet::read(data).unwrap();
}
