use simple_disk_cache::SimpleCache;
use textures::Texture;
use types::Uuid;

pub type TextureCache = SimpleCache<Uuid, Texture>;
