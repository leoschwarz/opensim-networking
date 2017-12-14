use image::GenericImage;
use jpeg2000;
use jpeg2000::decode::{Codec, ColorSpace, DecodeConfig};
use jpeg2000::error::DecodeError;
use logging::Log;
use textures::Texture;
use types::Uuid;

/// Extract JPEG2000 code stream.
pub fn extract_j2k(id: Uuid, raw_data: &[u8], log: &Log) -> Result<Texture, DecodeError> {
    let config = DecodeConfig {
        default_colorspace: Some(ColorSpace::SRGB),
        discard_level: 0,
    };
    let image =
        jpeg2000::decode::from_memory(raw_data, Codec::J2K, config, Some(log.slog_logger()))?;
    // TODO: Check if this is the right direction.
    Ok(Texture {
        id: id,
        width: image.width(),
        height: image.height(),
        data: image.to_rgba().into_raw(),
    })
}
