//! This file is here for debugging purposes and not a real example.

extern crate opensim_networking;
extern crate image;

use image::{GenericImage, ImageBuffer};
use std::fs::File;

use opensim_networking::packet::Packet;
use opensim_networking::layer_data::Surface;
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
    // TODO rename once naming is clear
    let patches = Surface::extract_message(&msg).unwrap();

    // Generate a 16x16 bitmap displaying the received height map.
    for (i_layer, ref patch) in patches.iter().enumerate() {
        println!("(x,y) = {:?}", patch.patch_position());

        let mut min = 1e20;
        let mut max = -1e20;
        for val in patch.data().iter() {
            if val < &min {
                min = *val;
            }
            if val > &max {
                max = *val;
            }
        }

        let image = ImageBuffer::from_fn(16, 16, |x, y| {
            let pixel = 255. * (patch.data()[(x as usize, y as usize)] - min) / (max - min);
            image::Luma([pixel as u8])
        });
        image.save(format!("layerdata/layer_{:2}.png", i_layer)).unwrap();
    }
}
