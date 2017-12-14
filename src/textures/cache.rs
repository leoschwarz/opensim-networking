use rmp_serde;
use std::fs::File;
use std::path::PathBuf;
use textures::{Texture, TextureServiceError};
use types::Uuid;

// TODO: How is cache invalidation handled.

pub trait TextureCache {
    fn get_texture(&self, id: &Uuid) -> Result<Option<Texture>, TextureServiceError>;
    fn put_texture(&self, id: &Uuid, texture: &Texture) -> Result<(), TextureServiceError>;
}

/// Just stores everything and never cleans up.
///
/// TODO: In the future it could do something like the original viewer and
///       remove old items once the disk is full, however most likely a better
///       cache architecture should be followed anyway.
///
///       Relevant reading:
///       - http://wiki.secondlife.com/wiki/Talk:Texture_cache
///       - [more on efficient key value storage and caching]
pub struct GreedyCache {
    dir: PathBuf,
}

impl GreedyCache {
    fn texture_path(&self, id: &Uuid) -> PathBuf {
        let s = format!("{}", id);
        let mut chars = s.chars();
        let dir_1 = format!("{}", chars.next().unwrap());
        let dir_2 = format!("{}", chars.next().unwrap());

        self.dir.join(dir_1).join(dir_2)
    }
}

impl TextureCache for GreedyCache {
    fn get_texture(&self, id: &Uuid) -> Result<Option<Texture>, TextureServiceError> {
        let path = self.texture_path(id);
        if path.exists() {
            let file = File::open(path)?;
            let texture_data: Texture = rmp_serde::decode::from_read(file)
                .map_err(|e| TextureServiceError::DecodeError(Box::new(e)))?;
            Ok(Some(texture_data))
        } else {
            Ok(None)
        }
    }

    fn put_texture(&self, id: &Uuid, texture: &Texture) -> Result<(), TextureServiceError> {
        let path = self.texture_path(id);
        let mut file = File::create(path)?;
        rmp_serde::encode::write(&mut file, texture)
            .map_err(|e| TextureServiceError::DecodeError(Box::new(e)))?;
        Ok(())
    }
}

/*
#[derive(Serialize, Deserialize)]
struct CacheData {
    capacity: u32,
    cache_entries: LinkedHashMap<Uuid, ()>,
}

/// A simple cache implementor for testing purposes.
///
/// Implements a LRU cache where every entry corresponds to a file on disk.
pub struct SimpleTextureCache {
    dir: PathBuf,
    data: Mutex<CacheData>,
}

impl SimpleTextureCache {
}

impl TextureCache for SimpleTextureCache {
    fn get_texture(&self, id: &Uuid) -> Result<Texture, TextureServiceError> {
        let mut data = self.data.lock();
        //data.cache_entries.
    }

    fn put_texture(&self, id: &Uuid, texture: &Texture) -> Result<(), TextureServiceError>;
}
*/
