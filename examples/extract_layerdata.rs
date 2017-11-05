//! This file is here for debugging purposes and not a real example.

extern crate opensim_networking;

use opensim_networking::packet::Packet;
use opensim_networking::layer_data::LayerPatch;
use opensim_networking::messages::MessageInstance;

fn main()
{
    let data = include_bytes!("layerdata.bin");
    let packet = Packet::read(data).unwrap();
    let msg_instance = packet.message;
    let msg = match msg_instance {
        MessageInstance::LayerData(data) => data,
        _ => panic!("wrong message instance"),
    };
    LayerPatch::extract_message(&msg).unwrap();
}
