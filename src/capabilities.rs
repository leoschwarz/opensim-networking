//! Implementation of the Capabilities protocol.

use std::io::Read;
// use futures::{Future, Stream};
// use tokio_core::reactor::Handle;
use llsd;
use reqwest;
use types::Url;

#[derive(Debug)]
pub struct Urls {
    pub get_texture: Url,
    // TODO: add more.
}

#[derive(Debug)]
pub struct Capabilities {
    urls: Urls,
}

impl Capabilities {
    // TODO: Consider implementing this using async IO.
    pub fn make_request(seed_caps_uri: Url) -> Result<Capabilities, String> {
        let client = reqwest::Client::new();
        let mut response = client
            .get(seed_caps_uri)
            .send()
            .map_err(|_| "Request failed.".to_string())?;

        if response.status().is_success() {
            let mut raw_data = Vec::new();
            response
                .read_to_end(&mut raw_data)
                .map_err(|_| "read failure".to_string())?;
            let val = llsd::read_value(&raw_data[..]).map_err(|_| "Invalid LLSD".to_string())?;

            match val {
                llsd::data::Value::Map(mut map) => {
                    let get_texture = map.remove("GetTexture")
                        .and_then(|v| v.scalar())
                        .and_then(|s| s.as_uri())
                        .and_then(|u| u.parse().ok())
                        .ok_or_else(|| "No GetTexture cap.".to_string())?;

                    Ok(Capabilities {
                        urls: Urls {
                            get_texture: get_texture,
                        },
                    })
                }
                _ => Err("LLSD is not a map.".to_string()),
            }
        } else {
            Err("Response is error.".to_string())
        }
    }
}
