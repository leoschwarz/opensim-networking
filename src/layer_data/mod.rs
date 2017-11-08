// TODO: This should probably moved to its own crate at a later time.

use bitreader::{BitReader, BitReaderError};

use messages::all::LayerData;

const END_OF_PATCH: u8 = 97u8;

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ExtractSurfaceError")]
#[error_chain(result = "")]
pub enum ExtractSurfaceErrorKind {
    #[error_chain(foreign)]
    BitReader(BitReaderError),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unknown layer type""#)]
    #[error_chain(display = r#"|code| write!(f, "unknown layer type: {}", code)"#)]
    UnknownLayerType(u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerKind {
    Land,
    Wind,
    Cloud,
    Water,
    AuroraLand,
    AuroraWind,
    AuroraCloud,
    AuroraWater,
}

impl LayerKind {
    fn is_large_patch(&self) -> bool {
        match *self {
            LayerKind::Land => false,
            _ => unimplemented!(), // TODO
        }
    }
}

pub struct Surface {
    cell_count_per_edge: u32,
    cell_width: f32,
    surface_width: f32,
}

impl Surface {
    pub fn extract_message(msg: &LayerData) -> Result<(), ExtractSurfaceError> {
        let kind = match msg.layer_id.type_ {
            b'L' => LayerKind::Land,
            b'7' => LayerKind::Wind,
            b'8' => LayerKind::Cloud,
            b'W' => LayerKind::Water,
            b'M' => LayerKind::AuroraLand,
            b'X' => LayerKind::AuroraWind,
            b'9' => LayerKind::AuroraCloud,
            b':' => LayerKind::AuroraWater,
            code => return Err(ExtractSurfaceErrorKind::UnknownLayerType(code).into()),
        };
        println!("kind : {:?}", kind);
        Self::extract(&msg.layer_data.data[..], kind.is_large_patch())
    }

    fn extract(data: &[u8], large_patch: bool) -> Result<(), ExtractSurfaceError> {
        let mut reader = BitReader::new(data);

        // Read patch_group_header
        let stride = reader.read_u16(16)?; // TODO byte order
        // TODO: Can patch_i and patch_j be larger than this?
        // Because this is what's currently happening in the test, patch_size=16, but patch_i,j
        // are in the range {0,...LARGE_PATCH_SIZE-1=31}
        let patch_size = reader.read_u8(8)? as usize;
        let layer_type = reader.read_u8(8)?;

        println!("stride:     0x{:X}", stride);
        println!("patch_size: {}", patch_size);
        println!("layer_type: 0x{:X}", layer_type);

        loop {
            // Read patch_header
            let quantity_wbits = reader.read_u8(8)?;
            if quantity_wbits == END_OF_PATCH {
                break;
            }
            let dc_offset = reader.read_u32(32)?; // TODO byte order
            let range = reader.read_u16(16)?; // TODO byte order
            let patch_ids = if large_patch {
                reader.read_u32(32)?
            } else {
                reader.read_u32(10)?
            };

            let (patch_i, patch_j) = if large_patch {
                (patch_ids >> 16, patch_ids & 0xffff)
            } else {
                (patch_ids >> 5, patch_ids & 0x1f)
            };

            println!("=============== new patch ================ ");
            println!("quantity_wbits: 0x{:X}", quantity_wbits);
            println!("dc_offset:      0x{:X}", dc_offset);
            println!("range:          0x{:X}", range);
            println!("patch_ids:      0x{:X}", patch_ids);
            println!("patch_i:        {}", patch_i);
            println!("patch_j:        {}", patch_j);

            // Read patches.
            // TODO: in the original code this is always initialized to LARGE_PATCH_SIZE^2 items.
            // TODO: With this code we will always write at least as many items, but assert that we
            // don't end up writing more items.
            let mut patch_data = Vec::<i32>::new();
            'read_patch_data: for i in 0..patch_size * patch_size {
                let exists = reader.read_bool()?;
                if exists {
                    let not_eob = reader.read_bool()?;
                    if not_eob {
                        // Read the item.
                        let sign = if reader.read_bool()? { -1 } else { 1 };
                        let value = reader.read_u8(8)? as i32;
                        patch_data.push(sign * value);
                    } else {
                        for _ in i..patch_size * patch_size {
                            patch_data.push(0);
                        }
                        break 'read_patch_data;
                    }
                } else {
                    patch_data.push(0);
                }
            }
            println!("patch_data.len(): {}", patch_data.len());
        }

        Ok(())
    }
}
