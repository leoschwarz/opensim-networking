//! This file is here for debugging purposes and not a real example.

extern crate opensim_networking;
extern crate image;
extern crate nalgebra;

use image::{GenericImage, ImageBuffer};
use nalgebra::DMatrix;
use std::fs::File;

use opensim_networking::packet::Packet;
use opensim_networking::layer_data::{Patch, self};
use opensim_networking::messages::MessageInstance;

fn extract_patches(raw_messages: Vec<Vec<u8>>) -> Vec<Patch> {
    raw_messages.iter().flat_map(|data| {
        let packet = Packet::read(&data[..]).unwrap();
        let msg_instance = packet.message;
        let msg = match msg_instance {
            MessageInstance::LayerData(data) => data,
            _ => panic!("wrong message instance"),
        };
        layer_data::extract_land_patch(&msg).unwrap()
    }).collect()
}

fn write_image(patches: Vec<Patch>, region_size: usize, image_path: &str){
    println!("start generating image: {}", image_path);

    // Find global min and max values.
    let mut min = 1e20;
    let mut max = -1e20;
    for ref patch in patches.iter() {
        /*
        for val in patch.data().iter() {
            if val < &min {
                min = *val;
            }
            if val > &max {
                max = *val;
            }
        }
        */
        // TODO: values are not exactly the same...
        // patch.z_* -> (0.5, 24.88) ; for val ... -> (0.9452, 24.9975)
        if patch.z_min() < min {
            min = patch.z_min();
        }
        if max < patch.z_max() {
            max = patch.z_max();
        }
    }
    println!("global max: {}", max);
    println!("global min: {}", min);

    // Extract the full heightmap.
    let mut heightmap: DMatrix<f32> = DMatrix::from_element(region_size, region_size, 0.);
    for patch in patches {
        // TODO: Handle coordinate system correctly.
        let (patch_x, patch_y) = patch.patch_position();
        println!("patch_pos: ({}, {})", patch_x, patch_y);
        let offset_x = (patch_x * patch.side_length()) as usize;
        let offset_y = (patch_y * patch.side_length()) as usize;

        for x in 0..(patch.side_length() as usize) {
            for y in 0..(patch.side_length() as usize) {
                heightmap[(x + offset_x, y + offset_y)] = patch.data()[(x, y)];
            }
        }
    }

    // Create the image.
    println!("region_size: {}", region_size);
    let image = ImageBuffer::from_fn(region_size as u32, region_size as u32, |x, y| {
        let pixel = 255. * (heightmap[(x as usize, y as usize)] - min) / (max - min);
        image::Luma([pixel as u8])
    });
    image.save(image_path).unwrap();
    println!("image has been written: {}", image_path);
}

fn main() {
    let data_land = get_data_land();
    let data_varland = get_data_varland();

    let patches_land = extract_patches(data_land);
    write_image(patches_land, 256, "layer_land.png");

    let patches_varland = extract_patches(data_varland);
    write_image(patches_varland, 1024, "layer_varland.png");
    //write_image(patches_varland, 2048, "layer_varland.png");
}

fn get_data_land() -> Vec<Vec<u8>> {
    vec![
        include_bytes!("data/layer_land/00000018.bin").to_vec(),
        include_bytes!("data/layer_land/00000020.bin").to_vec(),
        include_bytes!("data/layer_land/00000025.bin").to_vec(),
        include_bytes!("data/layer_land/00000027.bin").to_vec(),
        include_bytes!("data/layer_land/00000029.bin").to_vec(),
        include_bytes!("data/layer_land/00000032.bin").to_vec(),
        include_bytes!("data/layer_land/00000035.bin").to_vec(),
        include_bytes!("data/layer_land/00000038.bin").to_vec(),
        include_bytes!("data/layer_land/00000040.bin").to_vec(),
        include_bytes!("data/layer_land/00000043.bin").to_vec(),
        include_bytes!("data/layer_land/00000045.bin").to_vec(),
        include_bytes!("data/layer_land/00000046.bin").to_vec(),
        include_bytes!("data/layer_land/00000060.bin").to_vec(),
        include_bytes!("data/layer_land/00000062.bin").to_vec()
    ]
}

fn get_data_varland() -> Vec<Vec<u8>> {
    vec![
        include_bytes!("data/layer_varland/00000138.bin").to_vec(),
        include_bytes!("data/layer_varland/00000140.bin").to_vec(),
        include_bytes!("data/layer_varland/00000142.bin").to_vec(),
        include_bytes!("data/layer_varland/00000145.bin").to_vec(),
        include_bytes!("data/layer_varland/00000148.bin").to_vec(),
        include_bytes!("data/layer_varland/00000149.bin").to_vec(),
        include_bytes!("data/layer_varland/00000151.bin").to_vec(),
        include_bytes!("data/layer_varland/00000153.bin").to_vec(),
        include_bytes!("data/layer_varland/00000155.bin").to_vec(),
        include_bytes!("data/layer_varland/00000158.bin").to_vec(),
        include_bytes!("data/layer_varland/00000160.bin").to_vec(),
        include_bytes!("data/layer_varland/00000162.bin").to_vec(),
        include_bytes!("data/layer_varland/00000164.bin").to_vec(),
        include_bytes!("data/layer_varland/00000166.bin").to_vec(),
        include_bytes!("data/layer_varland/00000168.bin").to_vec(),
        include_bytes!("data/layer_varland/00000170.bin").to_vec(),
        include_bytes!("data/layer_varland/00000171.bin").to_vec(),
        include_bytes!("data/layer_varland/00000172.bin").to_vec(),
        include_bytes!("data/layer_varland/00000175.bin").to_vec(),
        include_bytes!("data/layer_varland/00000176.bin").to_vec(),
        include_bytes!("data/layer_varland/00000178.bin").to_vec(),
        include_bytes!("data/layer_varland/00000181.bin").to_vec(),
        include_bytes!("data/layer_varland/00000183.bin").to_vec(),
        include_bytes!("data/layer_varland/00000185.bin").to_vec(),
        include_bytes!("data/layer_varland/00000187.bin").to_vec(),
        include_bytes!("data/layer_varland/00000188.bin").to_vec(),
        include_bytes!("data/layer_varland/00000191.bin").to_vec(),
        include_bytes!("data/layer_varland/00000192.bin").to_vec(),
        include_bytes!("data/layer_varland/00000195.bin").to_vec(),
        include_bytes!("data/layer_varland/00000197.bin").to_vec(),
        include_bytes!("data/layer_varland/00000199.bin").to_vec(),
        include_bytes!("data/layer_varland/00000201.bin").to_vec(),
        include_bytes!("data/layer_varland/00000204.bin").to_vec(),
        include_bytes!("data/layer_varland/00000205.bin").to_vec(),
        include_bytes!("data/layer_varland/00000207.bin").to_vec(),
        include_bytes!("data/layer_varland/00000208.bin").to_vec(),
        include_bytes!("data/layer_varland/00000209.bin").to_vec(),
        include_bytes!("data/layer_varland/00000211.bin").to_vec(),
        include_bytes!("data/layer_varland/00000212.bin").to_vec(),
        include_bytes!("data/layer_varland/00000213.bin").to_vec(),
        include_bytes!("data/layer_varland/00000214.bin").to_vec(),
        include_bytes!("data/layer_varland/00000216.bin").to_vec(),
        include_bytes!("data/layer_varland/00000217.bin").to_vec(),
        include_bytes!("data/layer_varland/00000220.bin").to_vec(),
        include_bytes!("data/layer_varland/00000221.bin").to_vec(),
        include_bytes!("data/layer_varland/00000222.bin").to_vec(),
        include_bytes!("data/layer_varland/00000223.bin").to_vec(),
        include_bytes!("data/layer_varland/00000224.bin").to_vec(),
        include_bytes!("data/layer_varland/00000226.bin").to_vec(),
        include_bytes!("data/layer_varland/00000229.bin").to_vec(),
        include_bytes!("data/layer_varland/00000231.bin").to_vec(),
        include_bytes!("data/layer_varland/00000232.bin").to_vec(),
        include_bytes!("data/layer_varland/00000234.bin").to_vec(),
        include_bytes!("data/layer_varland/00000236.bin").to_vec(),
        include_bytes!("data/layer_varland/00000237.bin").to_vec(),
        include_bytes!("data/layer_varland/00000238.bin").to_vec(),
        include_bytes!("data/layer_varland/00000240.bin").to_vec(),
        include_bytes!("data/layer_varland/00000242.bin").to_vec(),
        include_bytes!("data/layer_varland/00000243.bin").to_vec(),
        include_bytes!("data/layer_varland/00000245.bin").to_vec(),
        include_bytes!("data/layer_varland/00000246.bin").to_vec(),
        include_bytes!("data/layer_varland/00000248.bin").to_vec(),
        include_bytes!("data/layer_varland/00000250.bin").to_vec(),
        include_bytes!("data/layer_varland/00000251.bin").to_vec(),
        include_bytes!("data/layer_varland/00000253.bin").to_vec(),
        include_bytes!("data/layer_varland/00000254.bin").to_vec(),
        include_bytes!("data/layer_varland/00000255.bin").to_vec(),
        include_bytes!("data/layer_varland/00000256.bin").to_vec(),
        include_bytes!("data/layer_varland/00000258.bin").to_vec(),
        include_bytes!("data/layer_varland/00000261.bin").to_vec(),
    ]
}
