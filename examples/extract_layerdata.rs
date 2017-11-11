//! This file is here for debugging purposes and not a real example.

extern crate opensim_networking;
extern crate image;
extern crate nalgebra;

use image::{GenericImage, ImageBuffer};
use nalgebra::DMatrix;
use std::fs::File;

use opensim_networking::packet::Packet;
use opensim_networking::layer_data::Surface;
use opensim_networking::messages::MessageInstance;

fn main() {
    let all_data = vec![
        include_bytes!("layerdata/00000018.bin").to_vec(),
        include_bytes!("layerdata/00000020.bin").to_vec(),
        include_bytes!("layerdata/00000025.bin").to_vec(),
        include_bytes!("layerdata/00000027.bin").to_vec(),
        include_bytes!("layerdata/00000029.bin").to_vec(),
        include_bytes!("layerdata/00000032.bin").to_vec(),
        include_bytes!("layerdata/00000035.bin").to_vec(),
        include_bytes!("layerdata/00000038.bin").to_vec(),
        include_bytes!("layerdata/00000040.bin").to_vec(),
        include_bytes!("layerdata/00000043.bin").to_vec(),
        include_bytes!("layerdata/00000045.bin").to_vec(),
        include_bytes!("layerdata/00000046.bin").to_vec(),
        include_bytes!("layerdata/00000060.bin").to_vec(),
        include_bytes!("layerdata/00000062.bin").to_vec()
    ];

    // TODO: I still don't understand the logic behind the patches per region,
    // if the region is really only 256m long, there are 4 patches per square meter.
    let mut heightmap: DMatrix<f32> = DMatrix::from_element(512, 512, 0.);

    for data in all_data {
        let packet = Packet::read(&data[..]).unwrap();
        let msg_instance = packet.message;
        let msg = match msg_instance {
            MessageInstance::LayerData(data) => data,
            _ => panic!("wrong message instance"),
        };
        // TODO rename once naming is clear
        let patches = Surface::extract_message(&msg).unwrap();

        // Generate a 16x16 bitmap displaying the received height map.
        for (i_layer, ref patch) in patches.iter().enumerate() {
            let (patch_x, patch_y) = patch.patch_position();
            println!("(x,y) = {:?}", patch.patch_position());

            let offset_x = (patch_x * 16) as usize;
            let offset_y = (patch_y * 16) as usize;

            // TODO I'm assuming the following coordinate system:
            // ^ y
            // |
            // |
            // +----> x

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

            for x in 0..patch.side_length() {
                for y in 0..patch.side_length() {
                    heightmap[(x + offset_x, y + offset_y)] = (patch.data()[(x, y)] - min) /
                        (max - min);
                }
            }
        }
    }

    let image = ImageBuffer::from_fn(512, 512, |x, y| {
        let pixel = 255. * heightmap[(x as usize, y as usize)];
        image::Luma([pixel as u8])
    });
    image.save("layerdata.png").unwrap();
}
