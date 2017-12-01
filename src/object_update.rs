// TODO

use util::bitsreader::{BytesReader, LittleEndian};
use types::{Quaternion, Uuid, Vector3, Vector4};

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
    pub media: u8,
    pub glow: f32,
    pub material_id: Uuid,
}

struct PartialFaceProperties {
    texture_id: Option<Uuid>,
    color: Option<Vector4<u8>>,
    repeat_u: Option<f32>,
    repeat_v: Option<f32>,
    offset_u: Option<f32>,
    offset_v: Option<f32>,
    rotation: Option<f32>,
    material: Option<u8>,
    media: Option<u8>,
    glow: Option<f32>,
    material_id: Option<Uuid>,
}

impl PartialFaceProperties {
    fn new() -> Self {
        PartialFaceProperties {
            texture_id: None,
            color: None,
            repeat_u: None,
            repeat_v: None,
            offset_u: None,
            offset_v: None,
            rotation: None,
            material: None,
            media: None,
            glow: None,
            material_id: None,
        }
    }

    fn full(self) -> Option<FaceProperties> {
        Some(FaceProperties {
            texture_id: self.texture_id?,
            color: self.color?,
            repeat_u: self.repeat_u?,
            repeat_v: self.repeat_v?,
            offset_u: self.offset_u?,
            offset_v: self.offset_v?,
            rotation: self.rotation?,
            material: self.material?,
            media: self.media?,
            glow: self.glow?,
            material_id: self.material_id?,
        })
    }

    fn complete(self, full: &FaceProperties) -> FaceProperties {
        FaceProperties {
            texture_id: self.texture_id.unwrap_or_else(|| full.texture_id.clone()),
            color: self.color.unwrap_or_else(|| full.color.clone()),
            repeat_u: self.repeat_u.unwrap_or(full.repeat_u),
            repeat_v: self.repeat_v.unwrap_or(full.repeat_v),
            offset_u: self.offset_u.unwrap_or(full.offset_u),
            offset_v: self.offset_v.unwrap_or(full.offset_v),
            rotation: self.rotation.unwrap_or(full.rotation),
            material: self.material.unwrap_or(full.material),
            media: self.media.unwrap_or(full.media),
            glow: self.glow.unwrap_or(full.glow),
            material_id: self.material_id.unwrap_or_else(|| full.material_id.clone()),
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

fn read_f32_B<R: BytesReader>(reader: &mut R) -> Result<f32, ::util::bitsreader::ReadError> {
    Ok(reader.read_bytes_u16::<LittleEndian>()? as f32 / 32768. * 2. * ::std::f32::consts::PI)
}

fn read_f32_C<R: BytesReader>(reader: &mut R) -> Result<f32, ::util::bitsreader::ReadError> {
    Ok(reader.read_bytes_u8()? as f32 / 255.)
}

fn read_uuid<R: BytesReader>(reader: &mut R) -> Result<Uuid, ::util::bitsreader::ReadError> {
    let mut bytes = [0u8; 16];
    reader.read_bytes_exact(&mut bytes)?;
    // TODO: remove unwrap
    Ok(Uuid::from_bytes(&bytes).unwrap())
}

// TODO: It's really bad that the Read bound has to be added here, but
// otherwise it really does not work due to some issue with generics.
fn read_texture_entry<R: BytesReader + ::std::io::Read>(
    reader: &mut R,
) -> Result<TextureEntry, ::util::bitsreader::ReadError> {
    // TODO

    // Fill these by reading the various property arrays.
    let mut default = PartialFaceProperties::new();
    let mut partial: Vec<PartialFaceProperties> = Vec::new();

    macro_rules! decode_prop_vec {
        (
            $f_name:ident = $read:expr
        )
            =>
        (
            default.$f_name = Some($read);
            loop {
                let (bitset, bitset_size) = read_face_bitfield(reader)?;
                if bitset == 0 {
                    break;
                }

                let value = $read;
                for i in 0..bitset_size {
                    if i >= partial.len() {
                        partial[i] = PartialFaceProperties::new();
                    }
                    partial[i].$f_name = Some(value.clone());
                }
            }
        )
    }

    decode_prop_vec!(texture_id = read_uuid(reader)?);
    decode_prop_vec!(
        color = Vector4::new(
            reader.read_bytes_u8()?,
            reader.read_bytes_u8()?,
            reader.read_bytes_u8()?,
            reader.read_bytes_u8()?
        )
    );

    decode_prop_vec!(repeat_u = reader.read_bytes_f32::<LittleEndian>()?);
    decode_prop_vec!(repeat_v = reader.read_bytes_f32::<LittleEndian>()?);
    decode_prop_vec!(offset_u = read_f32_A(reader)?);
    decode_prop_vec!(offset_v = read_f32_A(reader)?);
    decode_prop_vec!(rotation = read_f32_B(reader)?);
    decode_prop_vec!(material = reader.read_bytes_u8()?);
    decode_prop_vec!(media = reader.read_bytes_u8()?);
    decode_prop_vec!(glow = read_f32_C(reader)?);
    decode_prop_vec!(material_id = read_uuid(reader)?);

    // Note: At this point this should not be able to fail, since each default value
    //       should have been extracted or there should have been an early return of
    //       this function.
    let default = default.full().unwrap();
    Ok(
        partial
            .into_iter()
            .map(|item| item.complete(&default))
            .collect(),
    )
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
) -> Result<(u32, usize), ::util::bitsreader::ReadError> {
    // The decoded value.
    let mut face_bits = 0u32;
    // Number of bits read.
    let mut bits_read = 0usize;

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
