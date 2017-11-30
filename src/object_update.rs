// TODO

use util::bitsreader::{BytesReader, LittleEndian};
use types::{Quaternion, Vector3, Vector4, Uuid};

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

pub type TextureEntry = Vec<FaceProperties>;

#[derive(Debug)]
pub struct FaceProperties {
    /// The texture ID for this face.
    pub texture_id: Uuid,
    /// RGBA color value.
    pub color: Vector4<u8>,
    pub repeat_u: f32,
    pub repeat_v: f32,
    pub offset_u: f32,
    pub offset_v: f32,
    pub rotation: f32,
    pub material: u8,
    pub glow: f32,
    pub material_id: Uuid,
}

impl Default for FaceProperties {
    fn default() -> Self {
        FaceProperties {
            texture_id: Uuid::nil(),
            color: Vector4::new(0,0,0,0),
            repeat_u: 0.,
            repeat_v: 0.,
            offset_u: 0.,
            offset_v: 0.,
            rotation: 0.,
            material: 0,
            glow: 0.,
            material_id: Uuid::nil(),
        }
    }
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

fn read_f32_A<R: BytesReader>(reader: &mut R) -> Result<f32, ::util::bitsreader::ReadError> {
    Ok(reader.read_bytes_i16::<LittleEndian>()? as f32 / 32767.)
}

fn read_f32_B<R: BytesReader>(reader: &mut R) -> Result<f32, ::util::bitsreader::ReadError>
{
    Ok(reader.read_bytes_u16::<LittleEndian>()? as f32 / 32768. * 2. * ::std::f32::consts::PI)
}

fn read_f32_C<R: BytesReader>(reader: &mut R) -> Result<f32, ::util::bitsreader::ReadError>
{
    Ok(reader.read_bytes_u8()? as f32 / 255.)
}

fn read_texture_entry<R: BytesReader>(
    reader: &mut R
) -> Result<TextureEntry, ::util::bitsreader::ReadError> {



    unimplemented!()
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
