//! Implementation of the Capabilities protocol.

use std::io::{Read, Write};
// use futures::{Future, Stream};
// use tokio_core::reactor::Handle;
use llsd;
use reqwest;
use reqwest::header::{ContentLength, ContentType};
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
    fn build_request_body(val: &llsd::data::Value) -> Result<Vec<u8>, String> {
        let mut data = Vec::new();
        // data.write_all(&llsd::PREFIX_BINARY).unwrap();
        llsd::xml::write_doc(&mut data, val).unwrap();
        Ok(data)
    }

    // TODO: Consider implementing this using async IO.
    pub fn make_request(seed_caps_uri: Url) -> Result<Capabilities, String> {
        let requested_caps =
            llsd::data::Value::Array(vec![llsd::data::Value::new_string("GetTexture")]);

        let client = reqwest::Client::new();
        let request_body = Self::build_request_body(&requested_caps)?;
        let mut response = client
            .post(seed_caps_uri)
            .header(ContentType("application/llsd+xml".parse().unwrap()))
            .header(ContentLength(request_body.len() as u64))
            .body(request_body)
            .send()
            .map_err(|_| "Request failed.".to_string())?;

        if response.status().is_success() {
            let mut raw_data = Vec::new();

            {
                let c_type: &ContentType = response
                    .headers()
                    .get()
                    .ok_or_else(|| "Content type not specified.")?;
                if c_type.0.as_ref() != "application/xml" {
                    return Err(format!("wrong content type: {:?}", c_type));
                }
            }

            response
                .read_to_end(&mut raw_data)
                .map_err(|_| "read failure".to_string())?;
            let val = llsd::xml::read_value(&raw_data[..]).map_err(|_| "Invalid LLSD".to_string())?;

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
