use util::bitsreader::{BitsReader, BufBitsReader};
use layer_data::idct::{PatchSize, PatchTables};
use layer_data::{idct, LandLayerType, LayerType, Patch};

use byteorder::LittleEndian;

const END_OF_PATCH: u8 = 97u8;

lazy_static! {
    static ref TABLES_NORMAL: PatchTables<idct::NormalPatch>
        = PatchTables::compute();
    static ref TABLES_LARGE: PatchTables<idct::LargePatch>
        = PatchTables::compute();
}

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ExtractSurfaceError")]
#[error_chain(result = "")]
pub enum ExtractSurfaceErrorKind {
    #[error_chain(foreign)]
    BitReader(::util::bitsreader::ReadError),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unknown layer type""#)]
    #[error_chain(display = r#"|code| write!(f, "unknown layer type: {}", code)"#)]
    UnknownLayerType(u8),

    #[error_chain(custom)]
    WrongLayerType(LayerType),

    #[error_chain(custom)]
    UnsupportedPatchsize(u32),
}

#[derive(Debug)]
pub(super) struct PatchGroupHeader {
    pub stride: u32,
    pub patch_size: u32,
    pub layer_type: LayerType,
}

impl PatchGroupHeader {
    fn read<BR: BitsReader>(reader: &mut BR) -> Result<Self, ExtractSurfaceError> {
        // TODO: This is always set to the value 264, but to me it's unclear where I
        // need this.
        let stride = reader.read_bytes_u16::<LittleEndian>()?;

        // TODO: Can patch_i and patch_j be larger than this?
        // Because this is what's currently happening in the test, patch_size=16, but
        // patch_i,j are in the range {0,...LARGE_PATCH_PS-1=31}
        //
        // At this point I suspect (patch_x,patch_y) is not very relevant for decoding,
        // i.e. for large patches (patches_per_edge=32) patch_x being a u16
        // means it could go all the way up to 65565 which is even worse than
        // for normal size patches.
        let patch_size = reader.read_bytes_u8()?;
        let layer_type = reader.read_bytes_u8()?;

        Ok(PatchGroupHeader {
            stride: stride as u32,
            patch_size: patch_size as u32,
            layer_type: LayerType::from_code(layer_type)?,
        })
    }
}

#[derive(Debug)]
pub(super) struct PatchHeader {
    // also called prequant in LL and OpenSim code.
    pub quant: u32,
    pub word_bits: u32,
    pub dc_offset: f32,
    pub range: u16,
    pub patch_x: u32,
    pub patch_y: u32,
}

impl PatchHeader {
    fn read<BR: BitsReader>(
        reader: &mut BR,
        layer: LandLayerType,
    ) -> Result<Option<Self>, ExtractSurfaceError> {
        let quantity_wbits = reader.read_bytes_u8()?;
        if quantity_wbits == END_OF_PATCH {
            return Ok(None);
        }

        let quant = (quantity_wbits as u32 >> 4) + 2;
        let word_bits = (quantity_wbits as u32 & 0xf) + 2;

        let dc_offset = reader.read_bytes_f32::<LittleEndian>()?;
        let range = reader.read_bytes_u16::<LittleEndian>()?;

        let (patch_x, patch_y) = match layer {
            LandLayerType::Land => {
                let patchids = reader.read_bits_u32::<LittleEndian>(10)?;
                let x = patchids >> 5;
                let y = patchids & 0x1f;
                (x, y)
            }
            LandLayerType::VarLand => {
                let patchids = reader.read_bytes_u32::<LittleEndian>()?;
                let x = patchids >> 16;
                let y = patchids & 0xffff;
                (x, y)
            }
        };

        Ok(Some(PatchHeader {
            quant: quant,
            word_bits: word_bits,
            dc_offset: dc_offset,
            range: range,
            patch_x: patch_x,
            patch_y: patch_y,
        }))
    }
}

pub fn extract_land_patches(
    data: &[u8],
    expected_layer_type: LandLayerType,
) -> Result<Vec<Patch>, ExtractSurfaceError> {
    let mut reader = BufBitsReader::new(data);

    // Read patch_group_header
    let group_header = PatchGroupHeader::read(&mut reader)?;
    // TODO This assertion should not be nescessary.
    assert_eq!(
        group_header.layer_type.land_layer(),
        Some(expected_layer_type)
    );

    let mut decoded_patches = Vec::new();
    loop {
        // Read patch_header if there are more patches to be read.
        let header = match PatchHeader::read(&mut reader, expected_layer_type)? {
            Some(h) => h,
            None => return Ok(decoded_patches),
        };

        let patch = match group_header.patch_size {
            16 => decode_patch_data::<idct::NormalPatch, BufBitsReader>(
                &mut reader,
                &header,
                &TABLES_NORMAL,
            ),
            32 => decode_patch_data::<idct::LargePatch, BufBitsReader>(
                &mut reader,
                &header,
                &TABLES_LARGE,
            ),
            ps => Err(ExtractSurfaceErrorKind::UnsupportedPatchsize(ps).into()),
        }?;

        decoded_patches.push(patch);
    }
}

fn decode_patch_data<PS: PatchSize, BR: BitsReader>(
    reader: &mut BR,
    header: &PatchHeader,
    tables: &PatchTables<PS>,
) -> Result<Patch, ExtractSurfaceError> {
    // Read raw patch data.
    let mut patch_data = Vec::<i32>::new();
    for i in 0..PS::per_patch() {
        let exists = reader.read_bit_bool()?;
        if exists {
            let not_eob = reader.read_bit_bool()?;
            if not_eob {
                // Read the item.
                let sign = if reader.read_bit_bool()? { -1 } else { 1 };
                let value = reader.read_bits_u32::<LittleEndian>(header.word_bits as u8)? as i32;
                patch_data.push(sign * value);
            } else {
                for _ in i..PS::per_patch() {
                    patch_data.push(0);
                }
                break;
            }
        } else {
            patch_data.push(0);
        }
    }

    // Decompress the data.
    let data = idct::decompress_patch::<PS>(&patch_data, &header, &tables);
    Ok(Patch {
        size: PS::per_direction() as u32,
        z_min: header.dc_offset,
        z_max: (header.range as f32) - 1. + header.dc_offset,
        patch_pos: (header.patch_x, header.patch_y),
        data: data,
    })
}
