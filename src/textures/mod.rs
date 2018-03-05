//! Contains the texture manager.
use capabilities::Capabilities;
use futures::{self, Future, Stream};
use logging::Log;
use hyper;
use hyper::header::ContentType;
use slog::Logger;
use std::cell::RefCell;
use std::error::Error;
use std::io::Error as IoError;
use tokio_core::reactor::Handle;
use types::Uuid;
use url::Url;

mod cache;
mod decode;

use self::cache::*;

pub type GetTexture = Box<Future<Item = Texture, Error = TextureServiceError>>;

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
    caches: Vec<RefCell<TextureCache>>,
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
    pub fn register_cache(&mut self, cache: RefCell<TextureCache>) {
        self.caches.push(cache);
    }

    /// Get a texture by first checking the cache, then performing a network request
    /// if it was not found.
    pub fn get_texture(&self, id: &Uuid, handle: &Handle) -> GetTexture {
        // Get the texture from a cache if possible.
        // TODO: Currently this is performed with blocking IO.
        for cache_refcell in &self.caches {
            match cache_refcell.borrow_mut().get(id) {
                Ok(Some(t)) => return Box::new(futures::future::result(Ok(t))),
                _ => {}
            }
        }

        // Get the texture from the network instead.
        let url_res = self.get_texture
            .join(format!("?texture_id={}", id).as_str());
        let url = match url_res {
            Ok(u) => u,
            Err(_) => {
                return Box::new(futures::future::result(Err(
                    TextureServiceError::SimConfigError(format!(
                        "get_texture url: {}",
                        self.get_texture
                    )),
                )))
            }
        };

        let client = hyper::Client::new(handle);
        let logger = Logger::root(self.log.clone(), o!("texture request" => format!("{}",id)));
        debug!(logger, "request url: {:?}", url.clone());
        // TODO see: https://github.com/hyperium/hyper/issues/1219
        let uri: hyper::Uri = url.into_string().parse().unwrap();
        let response = client.get(uri);

        let id = id.clone();
        let log = self.log.clone();
        Box::new(
            response
                .map_err(|e| TextureServiceError::NetworkError(format!("{:?}", e)))
                .and_then(move |resp| {
                    debug!(logger, "received response");
                    let f: Box<
                        Future<Item = Texture, Error = TextureServiceError>,
                    > = if resp.status().is_success() {
                        let content_type = match resp.headers().get::<ContentType>() {
                            None => Err(TextureServiceError::NetworkError(
                                "No content type found.".to_string(),
                            )),
                            Some(ct) => {
                                println!("content_type: {}", ct);
                                // this should equal "image/x-j2c".

                                Ok(())
                            }
                        };

                        // TODO: This is bad for big textures!!!
                        Box::new(
                            resp.body()
                                .concat2()
                                .map_err(|e| TextureServiceError::NetworkError(format!("{:?}", e)))
                                .and_then(move |data| {
                                    // TODO: Perform the work on a thread pool!
                                    decode::extract_j2k(id, &data[..], log)
                                        .map_err(|e| TextureServiceError::DecodeError(Box::new(e)))
                                }),
                        )
                    } else {
                        Box::new(futures::future::result(Err(
                            TextureServiceError::NetworkError(format!(
                                "Sim returned status: {}",
                                resp.status()
                            )),
                        )))
                    };
                    f
                }),
        )
    }
}
