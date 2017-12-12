use jpeg2000;
use jpeg2000::error::DecodeError;
use jpeg2000::decode::{Codec, ColorSpace, DecodeConfig};
use textures::TextureData;
use image::GenericImage;

/// Extract JPEG2000 code stream.
pub fn extract_j2k(raw_data: &[u8]) -> Result<TextureData, DecodeError> {
    let config = DecodeConfig {
        default_colorspace: Some(ColorSpace::SRGB),
        discard_level: 0,
    };
    let image = jpeg2000::decode::from_memory(raw_data, Codec::J2K, config, None)?;
    // TODO: Check if this is the right direction.
    Ok(TextureData {
        width: image.width(),
        height: image.height(),
        data: image.to_rgba().into_raw(),
    })
}
