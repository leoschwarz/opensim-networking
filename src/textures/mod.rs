//! Contains the texture manager.
use capabilities::Capabilities;
use logging::Log;
use reqwest;
use std::error::Error;
use std::io::Read;
use std::io::Error as IoError;
use types::{Url, Uuid};

pub mod cache;
mod decode;

use self::cache::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Texture {
    id: Uuid,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(Debug)]
pub enum TextureServiceError {
    DecodeError(Box<Error + Send + Sync>),

    IoError(IoError),

    /// There is an error with the sim configuration.
    ///
    /// Note: This is supposed to only happen in that case,
    ///       but technically it might also be an issue somewhere
    ///       in our code.
    SimConfigError(String),

    /// There was an error during network communication.
    NetworkError(String),
}

impl From<IoError> for TextureServiceError {
    fn from(e: IoError) -> Self {
        TextureServiceError::IoError(e)
    }
}

impl From<::jpeg2000::error::DecodeError> for TextureServiceError {
    fn from(e: ::jpeg2000::error::DecodeError) -> Self {
        TextureServiceError::DecodeError(Box::new(e))
    }
}

pub struct TextureService {
    get_texture: Url,
    caches: Vec<Box<TextureCache>>,
    log: Log,
}

impl TextureService {
    pub fn new(caps: &Capabilities, log: Log) -> Self {
        TextureService {
            get_texture: caps.urls().get_texture.clone(),
            caches: Vec::new(),
            log: log,
        }
    }

    /// Register a TextureCache as the next layer in the cache hierarchy.
    ///
    /// Caches will be queried on lookup in the order they were inserted here.
    pub fn register_cache(&mut self, cache: Box<TextureCache>) {
        self.caches.push(cache);
    }

    /// Get a texture by first checking the cache, then performing a network request
    /// if it was not found.
    pub fn get_texture(&self, id: &Uuid) -> Result<Texture, TextureServiceError> {
        // Get the texture from a cache if possible.
        for cache in &self.caches {
            match cache.get_texture(id) {
                Ok(Some(t)) => return Ok(t),
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

            let texture = decode::extract_j2k(id.clone(), &data[..], &self.log)?;
            Ok(texture)
        } else {
            Err(TextureServiceError::NetworkError(
                format!("Sim returned status: {}", response.status()),
            ))
        }
    }
}
