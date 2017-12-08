//! Implementation of the Capabilities protocol.

use llsd::data::Value;
use std::io::Read;
// use futures::{Future, Stream};
// use tokio_core::reactor::Handle;
use llsd;
use reqwest;

type Url = String;

pub struct Urls {
    pub get_texture: Url,
    // TODO: add more.
}

struct Capabilities {
    urls: Urls,
}

impl Capabilities {
    // TODO: Consider implementing this using async IO.
    pub fn make_request(seed_caps_uri: ::reqwest::Url) -> Result<Capabilities, String> {
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
                        .and_then(|value| value.scalar())
                        .ok_or_else(|| "No GetTexture cap.".to_string())?;
                    Ok(Capabilities {
                        urls: Urls {
                            get_texture: get_texture
                                .as_uri()
                                .ok_or_else(|| "Wrong type".to_string())?,
                        },
                    })
                }
                _ => Err("LLSD is not a map.".to_string()),
            }
        } else {
            Err("Response is error.".to_string())
        }
    }

    /*
    pub fn make_request(handle: &Handle) -> Box<Future<Item=Capabilities, Error=String>> {
        let requested_caps = vec![Value::new_string("GetTexture")];
        let client = hyper::Client::new(handle);
        // TODO remove unwrap
        let base_url = "";

        Box::new(client.get(base_url.parse().unwrap()).then(|response| {
            response.and_then(|resp| {
                if resp.status().is_success() {
                    Box::new(resp.body().concat2().then(|chunk| {
                        if let Ok(chunk) = chunk {
                            // TODO: don't swallow LLSD error
                            llsd::read_value(chunk.as_ref()).map_err(|_| "Invalid LLSD".to_string())
                        } else {
                            Err("Concating chunks failed.".to_string())
                        }
                    }))
                } else {
                    Box::new(Err("Request was not successful.".to_string()))
                }
            })
        }))
    }
    */
}
