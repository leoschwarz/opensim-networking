extern crate opensim_networking;

use opensim_networking::packet::Packet;

#[test]
fn packet_appended_acks()
{
    let data = include_bytes!("data/appended_acks.bin");
    Packet::read(data).unwrap();
}
