// TODO

use util::bitsreader::{BytesReader, LittleEndian};
use types::{Quaternion, Vector3, Vector4};

#[derive(Debug)]
pub struct ObjectData {
    pub local_id: u32,
    pub state: u8,
    pub collision_plane: Option<Vector4<f32>>,
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub acceleration: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub angular_velocity: Vector3<f32>,
}

#[inline]
fn u16_to_float(value: u16, range_l: f32, range_r: f32) -> f32 {
    debug_assert!(range_l < range_r);
    let fvalue = (value as f32) / (range_r - range_l) + range_l;

    if fvalue.abs() < (range_r - range_l) / 255. {
        0.
    } else {
        fvalue
    }
}

#[inline]
fn read_u16f<R: BytesReader>(
    reader: &mut R,
    range_r: f32,
) -> Result<f32, ::util::bitsreader::ReadError> {
    Ok(u16_to_float(
        reader.read_bytes_u16::<LittleEndian>()?,
        -range_r,
        range_r,
    ))
}

pub fn read_object_data<R: BytesReader>(
    reader: &mut R,
) -> Result<ObjectData, ::util::bitsreader::ReadError> {
    let local_id = reader.read_bytes_u32::<LittleEndian>()?;
    let state = reader.read_bytes_u8()?;
    let collision_exists = reader.read_bytes_bool()?;
    let collision_plane = if collision_exists {
        Some(Vector4::new(
            reader.read_bytes_f32::<LittleEndian>()?,
            reader.read_bytes_f32::<LittleEndian>()?,
            reader.read_bytes_f32::<LittleEndian>()?,
            reader.read_bytes_f32::<LittleEndian>()?,
        ))
    } else {
        None
    };
    let position = Vector3::new(
        reader.read_bytes_f32::<LittleEndian>()?,
        reader.read_bytes_f32::<LittleEndian>()?,
        reader.read_bytes_f32::<LittleEndian>()?,
    );
    let velocity = Vector3::new(
        read_u16f(reader, 128.)?,
        read_u16f(reader, 128.)?,
        read_u16f(reader, 128.)?,
    );
    let acceleration = Vector3::new(
        read_u16f(reader, 64.)?,
        read_u16f(reader, 64.)?,
        read_u16f(reader, 64.)?,
    );
    let rotation = Quaternion::new(
        read_u16f(reader, 1.)?,
        read_u16f(reader, 1.)?,
        read_u16f(reader, 1.)?,
        read_u16f(reader, 1.)?,
    );
    let angular_vel = Vector3::new(
        read_u16f(reader, 64.)?,
        read_u16f(reader, 64.)?,
        read_u16f(reader, 64.)?,
    );

    Ok(ObjectData {
        local_id: local_id,
        state: state,
        collision_plane: collision_plane,
        position: position,
        velocity: velocity,
        acceleration: acceleration,
        rotation: rotation,
        angular_velocity: angular_vel,
    })
}

/*
fn read_texture_data() -> {

}
*/

fn read_face_bitfield<R: BytesReader>(
    reader: &mut R,
) -> Result<(u32, u32), ::util::bitsreader::ReadError> {
    // The decoded value.
    let mut face_bits = 0u32;
    // Number of bits read.
    let mut bits_read = 0u32;

    loop {
        let byte = reader.read_bytes_u8()? as u32;
        face_bits = (face_bits << 7) | (byte & 0x7f as u32);
        bits_read += 7;

        if byte & 0x80 != 0 {
            break;
        }
    }

    Ok((face_bits, bits_read))
}
