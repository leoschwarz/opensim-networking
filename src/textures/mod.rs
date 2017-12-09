//! Contains the texture manager.

pub mod cache {
    use types::Uuid;
    use textures::Texture;

    pub trait TextureCache {
        fn get_texture(id: &Uuid) -> Option<Texture>;
    }
}

pub struct Texture {
}
