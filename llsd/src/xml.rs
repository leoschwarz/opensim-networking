use data_encoding::{BASE64, Encoding};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::{BufRead, Read};

use data::*;

// TODO: Also figure out if this is even needed in OpenSim,
// since the alphabet here does not do the same as the example in the wiki,
// yet the Python implementation they linked does also follow the RFC 4648 alphabet.
lazy_static! {
    static ref BASE16: Encoding = {
        let mut spec = ::data_encoding::Specification::new();
        // https://tools.ietf.org/html/rfc4648#page-10
        spec.symbols.push_str("0123456789ABCDEF");
        spec.padding = None;
        spec.encoding().unwrap()
    };
}
/*
    // TODO: Figure out if this is even implemented in OpenSim.
    static ref BASE85: Encoding = {
        let mut spec = ::data_encoding::Specification::new();
        // https://de.wikipedia.org/wiki/Base85
        spec.symbols.push_str("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~");
        spec.padding = 
    }
*/

// TODO: see in relation to the binary module, that here we are actually reading away the xml
// header. So in the other case the reader should only be moved, if the expected data is actually
// found. This might actually complicate the implementation.
//
// → They define the MIME type "application/llsd+binary" for the other encoding, however it is not
// clear to me where the MIME type will be found in the LLUDP messages.

/// WARNING (TODO): Don't depend on this yet, this type will have to be refactored in the future.
#[derive(Debug, ErrorChain)]
#[error_chain(error = "ReadError")]
#[error_chain(result = "")]
pub enum ReadErrorKind {
    #[error_chain(foreign)]
    Xml(::quick_xml::errors::Error),

    #[error_chain(foreign)]
    BinaryDecode(::data_encoding::DecodeError),

    #[error_chain(custom)]
    UnexpectedEof,

    #[error_chain(custom)]
    UnexpectedText,

    #[error_chain(custom)]
    UnexpectedTag(String),

    #[error_chain(custom)]
    InvalidContainerType(String),

    #[error_chain(custom)]
    InvalidPartialValue(String),

    #[error_chain(custom)]
    InvalidStructure,

    #[error_chain(custom)]
    EmptyValue,

    /// Type conversion failed.
    #[error_chain(custom)]
    ConversionFailed,

    Msg(String),
}

#[derive(Debug, PartialEq)]
enum BinaryEncoding {
    Base16,
    Base64,
    Base85,
}

impl BinaryEncoding {
    fn enc(&self) -> &Encoding {
        match *self {
            BinaryEncoding::Base16 => &BASE16,
            BinaryEncoding::Base64 => &BASE64,
            BinaryEncoding::Base85 => unimplemented!(),
        }
    }
}

#[derive(Debug, PartialEq)]
enum PartialValue {
    Llsd,
    Array(Array),
    Map(Map),
    ScalarBinary(Option<Value>, BinaryEncoding),
    Scalar(ScalarType, Option<Value>),
    Key(Option<String>),
}

impl PartialValue {
    fn parse_name(name: &str) -> Result<Self, ReadError> {
        match name {
            "llsd" => Ok(PartialValue::Llsd),
            "array" => Ok(PartialValue::Array(Array::new())),
            "map" => Ok(PartialValue::Map(Map::new())),
            "boolean" => Ok(PartialValue::Scalar(ScalarType::Boolean, None)),
            "integer" => Ok(PartialValue::Scalar(ScalarType::Integer, None)),
            "real" => Ok(PartialValue::Scalar(ScalarType::Real, None)),
            "uuid" => Ok(PartialValue::Scalar(ScalarType::Uuid, None)),
            "string" => Ok(PartialValue::Scalar(ScalarType::String, None)),
            "date" => Ok(PartialValue::Scalar(ScalarType::Date, None)),
            "uri" => Ok(PartialValue::Scalar(ScalarType::Uri, None)),
            "binary" => Ok(PartialValue::ScalarBinary(None, BinaryEncoding::Base64)),
            "key" => Ok(PartialValue::Key(None)),
            t => Err(ReadErrorKind::InvalidPartialValue(t.to_string()).into()),
        }
    }

    fn extract(self) -> Result<Value, ReadError> {
        match self {
            PartialValue::Array(a) => Ok(Value::Array(a)),
            PartialValue::Map(m) => Ok(Value::Map(m)),
            PartialValue::ScalarBinary(val, _) => Ok(val.ok_or_else(|| {
                ReadError::from("Scalar binary was not specified.")
            })?),
            PartialValue::Scalar(_, Some(val)) => Ok(val),
            PartialValue::Scalar(_, None) => Err("Scalar was not specified.".into()),
            PartialValue::Llsd |
            PartialValue::Key(_) => {
                Err(
                    "Tried extracting PartialValue that cannot be extracted.".into(),
                )
            }
        }
    }
}

/// Taking a `Reader` as an argument, allows us to call this recursively.
pub fn read_value<B: BufRead>(reader: &mut Reader<B>) -> Result<Value, ReadError> {
    // Internal buffer of quick_xml Reader, which we can use for our purposes.
    let mut buf = Vec::new();
    let mut val_stack: Vec<PartialValue> = Vec::new();

    // Note: The reader takes care of checking that end elements match the open elements,
    //       so less sanity checking has to be done on our end.
    loop {
        match reader.read_event(&mut buf)? {
            Event::Start(ref e) => {
                let mut vt = PartialValue::parse_name(e.unescape_and_decode(reader)?.as_str())?;
                if vt == PartialValue::Llsd && val_stack.len() > 0 {
                    return Err(ReadErrorKind::InvalidStructure.into());
                } else if let PartialValue::ScalarBinary(_, ref mut enc) = vt {
                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_name = String::from_utf8_lossy(attr.key);
                        match attr_name.as_ref() {
                            "encoding" => {
                                let attr_value = String::from_utf8_lossy(attr.value);
                                *enc = match attr_value.as_ref() {
                                    "base16" => BinaryEncoding::Base16,
                                    "base85" => return Err("base85 unsupported.".into()),
                                    "base64" | _ => BinaryEncoding::Base64,
                                }
                            }
                            _ => {}
                        }
                    }
                }
                val_stack.push(vt);
            }
            Event::Text(ref e) => {
                if e.is_empty() {
                    continue;
                }

                // TODO: remove pop/push here later
                let mut target = val_stack.pop().unwrap();
                match target {
                    PartialValue::ScalarBinary(ref mut content, ref encoding) => {
                        let data = encoding.enc().decode(e.unescaped()?.as_ref())?;
                        let scalar = Scalar::Binary(data);
                        *content = Some(Value::Scalar(scalar));
                    }
                    PartialValue::Scalar(ref s_type, ref mut s_val) => {
                        let scalar = s_type.parse_scalar(e.escaped().as_ref()).ok_or_else(|| {
                            ReadError::from(ReadErrorKind::ConversionFailed)
                        })?;
                        *s_val = Some(Value::Scalar(scalar));
                    }
                    PartialValue::Key(ref mut key) => {
                        let string = e.unescape_and_decode(reader)?;
                        *key = Some(string);
                    }
                    _ => return Err("Only <key> and scalar elements can contain text.".into()),
                }
                val_stack.push(target);
            }
            Event::End(ref e) => {
                // Get the current value from the stack, this should never fail.
                let curr_val = val_stack.pop().ok_or_else(|| {
                    ReadError::from(ReadErrorKind::InvalidStructure)
                })?;

                // Get the previous value, this shouldn't fail in any valid LLSD XML instance.
                let mut prev_val = val_stack.pop().ok_or_else(|| {
                    ReadError::from(ReadErrorKind::InvalidStructure)
                })?;

                match prev_val {
                    PartialValue::Llsd => return Ok(curr_val.extract()?),
                    PartialValue::Array(ref mut a) => {
                        a.push(curr_val.extract()?);
                    }
                    PartialValue::Map(ref mut m) => {
                        // If the current value is a Key, skip, otherwise error.
                        match curr_val {
                            PartialValue::Key(_) => continue,
                            _ => return Err(ReadErrorKind::InvalidStructure.into()),
                        }
                    }
                    PartialValue::Scalar(_, _) |
                    PartialValue::ScalarBinary(_, _) => {
                        return Err(ReadErrorKind::InvalidStructure.into());
                    }
                    PartialValue::Key(Some(ref key)) => {
                        // If the preprevious value is a Map, insert, otherwise error.
                        let mut prev2_val = val_stack.pop().ok_or_else(|| {
                            ReadError::from(ReadErrorKind::InvalidStructure)
                        })?;
                        match prev2_val {
                            PartialValue::Map(ref mut m) => {
                                m.insert(key.clone(), curr_val.extract()?);
                            }
                            _ => return Err(ReadErrorKind::InvalidStructure.into()),
                        }
                        val_stack.push(prev2_val);
                    }
                    PartialValue::Key(None) => {
                        return Err("Empty key.".into());
                    }
                }

                val_stack.push(prev_val);
            }
            Event::Eof => return Err(ReadErrorKind::UnexpectedEof.into()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_value_direct(source: &'static str) -> Value {
        let mut reader = Reader::from_str(source);
        read_value(&mut reader).unwrap()
    }

    #[test]
    fn read_scalars() {
        let string = read_value_direct("<llsd><string>test</string></llsd>");
        assert_eq!(string, Value::Scalar(Scalar::String("test".to_string())));

        let int = read_value_direct("<llsd><integer>42</integer></llsd>");
        assert_eq!(int, Value::Scalar(Scalar::Integer(42)));

        let real = read_value_direct("<llsd><real>4.2</real></llsd>");
        assert_eq!(real, Value::Scalar(Scalar::Real(4.2)));
    }

    #[test]
    fn extract_1() {
        // Example from: http://wiki.secondlife.com/wiki/LLSD#Binary_Serialization
        let data = r#"<?xml version="1.0" encoding="UTF-8"?>
<llsd>
<map>
  <key>region_id</key>
    <uuid>67153d5b-3659-afb4-8510-adda2c034649</uuid>
  <key>scale</key>
    <string>one minute</string>
  <key>simulator statistics</key>
  <map>
    <key>time dilation</key><real>0.9878624</real>
    <key>sim fps</key><real>44.38898</real>
    <key>pysics fps</key><real>44.38906</real>
    <key>agent updates per second</key><real>nan</real>
    <key>lsl instructions per second</key><real>0</real>
    <key>total task count</key><real>4</real>
    <key>active task count</key><real>0</real>
    <key>active script count</key><real>4</real>
    <key>main agent count</key><real>0</real>
    <key>child agent count</key><real>0</real>
    <key>inbound packets per second</key><real>1.228283</real>
    <key>outbound packets per second</key><real>1.277508</real>
    <key>pending downloads</key><real>0</real>
    <key>pending uploads</key><real>0.0001096525</real>
    <key>frame ms</key><real>0.7757886</real>
    <key>net ms</key><real>0.3152919</real>
    <key>sim other ms</key><real>0.1826937</real>
    <key>sim physics ms</key><real>0.04323055</real>
    <key>agent ms</key><real>0.01599029</real>
    <key>image ms</key><real>0.01865955</real>
    <key>script ms</key><real>0.1338836</real>
  </map>
</map>
</llsd>"#;

        let mut reader = Reader::from_str(data);
        read_value(&mut reader).unwrap();
    }
}
