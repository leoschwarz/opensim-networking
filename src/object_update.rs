// TODO
use util::bitsreader::BitsReader;
use std::io::Read;
use byteorder::{ReadBytesExt, LittleEndian};

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

fn read_object_data<R: Read>(reader: &mut R) -> Result<ObjectData, ()> {
    let local_id = reader.read_u32::<LittleEndian>()?;
    let state = reader.read_u8()?;
    
}

fn read_texture_data() -> {

}

fn read_face_bitfield(reader: &mut BitsReader) -> (u32, u32) {
    // The decoded value.
    let mut face_bits = 0u32;
    // Number of bits read.
    let mut bits_read = 0u32;

    loop {
        let byte = reader.read_u8()?;
        face_bits = (face_bits << 7) | (byte & 0x7f as u32);
        bits_read += 7;

        if byte & 0x80 != 0 {
            break;
        }
    }

    (face_bits, bits_read)
}
