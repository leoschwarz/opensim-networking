//! Contains the texture manager.
use capabilities::Capabilities;
use reqwest;
use std::io::Read;
use types::{Url, Uuid};

pub mod cache {
    use types::Uuid;
    use textures::{Texture, TextureServiceError};

    pub trait TextureCache {
        fn get_texture(&self, id: &Uuid) -> Result<Texture, TextureServiceError>;
    }
}

use self::cache::*;

pub struct Texture {
    id: Uuid,
    data: Vec<u8>,
}

#[derive(Debug)]
pub enum TextureServiceError {
    /// The requested texture was not found.
    NotFound,

    /// There is an error with the sim configuration.
    ///
    /// Note: This is supposed to only happen in that case,
    ///       but technically it might also be an issue somewhere
    ///       in our code.
    SimConfigError(String),

    /// There was an error during network communication.
    NetworkError(String),
}

pub struct TextureService {
    get_texture: Url,
    caches: Vec<Box<TextureCache>>,
}

impl TextureService {
    pub fn new(caps: &Capabilities) -> Self {
        TextureService {
            get_texture: caps.urls().get_texture.clone(),
            caches: Vec::new(),
        }
    }

    pub fn register_cache(&mut self, cache: Box<TextureCache>) {
        self.caches.push(cache);
    }

    pub fn get_texture(&self, id: &Uuid) -> Result<Texture, TextureServiceError> {
        // Get the texture from a cache if possible.
        for cache in &self.caches {
            match cache.get_texture(id) {
                Ok(t) => return Ok(t),
                _ => {}
            }
        }

        // Get the texture from the network instead.
        let url = self.get_texture
            .join(format!("/?texture_id={}", id).as_str())
            .map_err(|_| {
                TextureServiceError::SimConfigError(
                    format!("get_texture url: {}", self.get_texture),
                )
            })?;

        // TODO: Async IO!!!
        let client = reqwest::Client::new();
        let mut response = client
            .get(url)
            .send()
            .map_err(|e| TextureServiceError::NetworkError(format!("{}", e)))?;
        if response.status().is_success() {
            // TODO: This is bad for big textures!!!
            let mut data = Vec::new();
            response.read_to_end(&mut data).unwrap();

            Ok(Texture {
                id: id.clone(),
                data: data,
            })
        } else {
            Err(TextureServiceError::NetworkError(
                format!("Sim returned status: {}", response.status()),
            ))
        }
    }
}
