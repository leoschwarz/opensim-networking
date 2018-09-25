//! Implementation of the Capabilities protocol.

use futures::prelude::{await, *};
use hyper;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
use llsd;
use tokio_core::reactor::Handle;
use url::Url;

#[derive(Clone, Debug)]
pub struct Urls {
    pub get_texture: Url,
    // TODO: add more.
}

#[derive(Clone, Debug)]
pub struct Capabilities {
    urls: Urls,
}

#[derive(Debug, Fail)]
pub enum CapabilitiesError {
    #[fail(display = "capabilities error: {}", 0)]
    Msg(String),
}

impl Capabilities {
    #[async]
    fn build_request_body(val: llsd::data::Value) -> Result<Vec<u8>, CapabilitiesError> {
        let mut data = Vec::new();
        // data.write_all(&llsd::PREFIX_BINARY).unwrap();
        llsd::xml::write_doc(&mut data, &val).unwrap();
        Ok(data)
    }

    pub fn urls(&self) -> &Urls {
        &self.urls
    }

    #[async]
    pub fn setup_capabilities(
        seed_caps_uri: hyper::Uri,
        handle: Handle,
    ) -> Result<Capabilities, CapabilitiesError> {
        let requested_caps =
            llsd::data::Value::Array(vec![llsd::data::Value::new_string("GetTexture")]);

        let client = hyper::Client::new();
        let request_body = await!(Self::build_request_body(requested_caps))?;
        let mut request = hyper::Request::post(seed_caps_uri)
            .header(CONTENT_TYPE, "application/llsd+xml")
            .header(CONTENT_LENGTH, request_body.len())
            .body(hyper::Body::from(request_body))
            .map_err(|e| CapabilitiesError::Msg(format!("Constructing request failed: {}", e)))?;
        let response = await!(client.request(request))
            .map_err(|_| CapabilitiesError::Msg("Request failed.".into()))?;

        if response.status().is_success() {
            {
                let c_type = response
                    .headers()
                    .get(CONTENT_TYPE)
                    .ok_or_else(|| CapabilitiesError::Msg("Content type not specified.".into()))?;
                if c_type
                    .to_str()
                    .map_err(|e| CapabilitiesError::Msg(format!("{}", e)))?
                    != "application/xml"
                {
                    return Err(CapabilitiesError::Msg(format!(
                        "wrong content type: {:?}",
                        c_type
                    )));
                }
            }

            // Read full response body.
            let raw_data = await!(
                response
                    .into_body()
                    .concat2()
                    .map_err(|_| CapabilitiesError::Msg("Collecting body failed.".into()))
            )?;
            let val = llsd::xml::read_value(&raw_data[..])
                .map_err(|_| CapabilitiesError::Msg("Invalid LLSD".to_string()))?;

            match val {
                llsd::data::Value::Map(mut map) => {
                    let get_texture = map
                        .remove("GetTexture")
                        .and_then(|v| v.scalar())
                        .and_then(|s| s.as_uri())
                        .and_then(|u| u.ok())
                        .ok_or_else(|| CapabilitiesError::Msg("No GetTexture cap.".into()))?;

                    Ok(Capabilities {
                        urls: Urls {
                            get_texture: get_texture,
                        },
                    })
                }
                _ => Err(CapabilitiesError::Msg("LLSD is not a map.".into())),
            }
        } else {
            Err(CapabilitiesError::Msg("Response is error.".into()))
        }
    }
}
