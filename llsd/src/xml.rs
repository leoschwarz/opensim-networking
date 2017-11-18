//! Handle XML representation of LLSD data.

use data_encoding::{BASE64, Encoding};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::BufRead;

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
    static ref NULL_DATE: Date = {
        use chrono::{Utc, NaiveDateTime};
        let naive = NaiveDateTime::from_timestamp(0, 0);
        Date::from_utc(naive, Utc)
    };
}

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
}

impl BinaryEncoding {
    fn enc(&self) -> &Encoding {
        match *self {
            BinaryEncoding::Base16 => &BASE16,
            BinaryEncoding::Base64 => &BASE64,
        }
    }
}

#[derive(Debug, PartialEq)]
enum PartialValue {
    Llsd,
    Array(Array),
    Map(Map),
    ScalarBinary(Option<Value>, BinaryEncoding),
    Scalar(ScalarType, Value),
    Key(Option<String>),
}

impl PartialValue {
    fn parse_name(name: &str) -> Result<Self, ReadError> {
        // Scalars are initialized with a default value in case they are actually an empty tag.
        match name {
            "llsd" => Ok(PartialValue::Llsd),
            "array" => Ok(PartialValue::Array(Array::new())),
            "map" => Ok(PartialValue::Map(Map::new())),
            "boolean" => Ok(PartialValue::Scalar(
                ScalarType::Boolean,
                Value::new_boolean(false),
            )),
            "integer" => Ok(PartialValue::Scalar(
                ScalarType::Integer,
                Value::new_integer(0),
            )),
            "real" => Ok(PartialValue::Scalar(ScalarType::Real, Value::new_real(0.))),
            "uuid" => Ok(PartialValue::Scalar(
                ScalarType::Uuid,
                Value::new_uuid(Uuid::nil()),
            )),
            "string" => Ok(PartialValue::Scalar(
                ScalarType::String,
                Value::new_string(""),
            )),
            "date" => Ok(PartialValue::Scalar(
                ScalarType::Date,
                Value::new_date(NULL_DATE.clone()),
            )),
            "uri" => Ok(PartialValue::Scalar(ScalarType::Uri, Value::new_uri(""))),
            "binary" => Ok(PartialValue::ScalarBinary(None, BinaryEncoding::Base64)),
            "key" => Ok(PartialValue::Key(None)),
            "undef" => Ok(PartialValue::Scalar(
                ScalarType::Undefined,
                Value::Scalar(Scalar::Undefined),
            )),
            t => Err(ReadErrorKind::InvalidPartialValue(t.to_string()).into()),
        }
    }

    fn extract(self) -> Result<Value, ReadError> {
        match self {
            PartialValue::Array(a) => Ok(Value::Array(a)),
            PartialValue::Map(m) => Ok(Value::Map(m)),
            PartialValue::ScalarBinary(val, _) => Ok(val.unwrap_or_else(
                || Value::new_binary(Vec::new()),
            )),
            PartialValue::Scalar(_, val) => Ok(val),
            PartialValue::Llsd |
            PartialValue::Key(_) => {
                Err(
                    "Tried extracting PartialValue that cannot be extracted.".into(),
                )
            }
        }
    }
}

pub fn read_value<B: BufRead>(buf_reader: B) -> Result<Value, ReadError> {
    // Internal buffer of quick_xml Reader, which we can use for our purposes.
    let mut buf = Vec::new();
    let mut val_stack: Vec<PartialValue> = Vec::new();

    let mut reader = Reader::from_reader(buf_reader);

    // Note: The reader takes care of checking that end elements match the open elements,
    //       so less sanity checking has to be done on our end.
    loop {
        match reader
            // Needed because otherwise empty string text events are emitted,
            // probably for whitespace between elements?
            //
            // TODO: But this might mess up strings which should contain whitespace.
            .trim_text(true)
            // Needed so we can extract the correct default values for numbers for instance.
            .expand_empty_elements(true)
            .read_event(&mut buf)? {
            Event::Start(ref e) => {
                let name_decoded = e.unescape_and_decode(&mut reader)?;
                let name = name_decoded.split_whitespace().next().unwrap();

                let mut vt = PartialValue::parse_name(name)?;
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
                // TODO: remove pop/push here later
                let mut target = val_stack.pop().unwrap();
                match target {
                    PartialValue::ScalarBinary(ref mut content, ref encoding) => {
                        let data = encoding.enc().decode(e.unescaped()?.as_ref())?;
                        let scalar = Scalar::Binary(data);
                        *content = Some(Value::Scalar(scalar));
                    }
                    PartialValue::Scalar(ref s_type, ref mut s_val) => {
                        let scalar = s_type.parse_scalar(e.unescaped()?.as_ref()).ok_or_else(
                            || {
                                ReadError::from(ReadErrorKind::ConversionFailed)
                            },
                        )?;
                        *s_val = Value::Scalar(scalar);
                    }
                    PartialValue::Key(ref mut key) => {
                        let string = e.unescape_and_decode(&mut reader)?;
                        *key = Some(string);
                    }
                    _ => return Err("Only <key> and scalar elements can contain text.".into()),
                }
                val_stack.push(target);
            }
            Event::End(_) => {
                // Get the current value from the stack, this should never fail.
                let curr_val = val_stack.pop().ok_or_else(|| {
                    ReadError::from(ReadErrorKind::InvalidStructure)
                })?;

                // Get the previous value, this shouldn't fail in any valid LLSD XML instance.
                let prev_val = val_stack.pop().ok_or_else(|| {
                    ReadError::from(ReadErrorKind::InvalidStructure)
                })?;

                match prev_val {
                    PartialValue::Llsd => return Ok(curr_val.extract()?),
                    PartialValue::Array(mut a) => {
                        a.push(curr_val.extract()?);
                        val_stack.push(PartialValue::Array(a));
                    }
                    PartialValue::Map(_) => {
                        // If the current value is a Key, skip, otherwise error.
                        match curr_val {
                            PartialValue::Key(_) => {
                                val_stack.push(prev_val);
                                val_stack.push(curr_val);
                            }
                            _ => return Err(ReadErrorKind::InvalidStructure.into()),
                        }
                    }
                    PartialValue::Scalar(_, _) |
                    PartialValue::ScalarBinary(_, _) => {
                        return Err(ReadErrorKind::InvalidStructure.into());
                    }
                    PartialValue::Key(Some(key)) => {
                        // If the preprevious value is a Map, insert, otherwise error.
                        let mut prev2_val = val_stack.pop().ok_or_else(|| {
                            ReadError::from(ReadErrorKind::InvalidStructure)
                        })?;
                        match prev2_val {
                            PartialValue::Map(ref mut m) => {
                                m.insert(key, curr_val.extract()?);
                            }
                            _ => return Err(ReadErrorKind::InvalidStructure.into()),
                        }
                        val_stack.push(prev2_val);
                    }
                    PartialValue::Key(None) => {
                        return Err("Empty key.".into());
                    }
                };

            }
            Event::Eof => return Err(ReadErrorKind::UnexpectedEof.into()),
            _ => {}
        }
    }
}

/// Most of the tests examples were taken from libOpenMetaverse,
/// however actual code was not copied.
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use std::io::Cursor;

    fn read_value_direct(source: &'static str) -> Value {
        let reader = Cursor::new(source);
        read_value(reader).unwrap()
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
    fn read_strings() {
        let value = read_value_direct(
            "<llsd><array>
                <string>test</string>
                <string>&lt; &gt; &amp; &apos; &quot;</string>
                <string/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 3);
        assert_eq!(array[0], Value::new_string("test"));
        assert_eq!(array[1], Value::new_string("< > & ' \""));
        assert_eq!(array[2], Value::new_string(""));
    }

    #[test]
    fn read_integers() {
        let value = read_value_direct(
            "<llsd><array>
                 <integer>2147483647</integer>
                 <integer>-2147483648</integer>
                 <integer>0</integer>
                 <integer>013</integer>
                 <integer/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 5);
        assert_eq!(array[0], Value::new_integer(2147483647));
        assert_eq!(array[1], Value::new_integer(-2147483648));
        assert_eq!(array[2], Value::new_integer(0));
        assert_eq!(array[3], Value::new_integer(13));
        assert_eq!(array[4], Value::new_integer(0));
    }

    #[test]
    fn read_uuid() {
        let value = read_value_direct(
            "<llsd><array><uuid>d7f4aeca-88f1-42a1-b385-b9db18abb255</uuid><uuid/></array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 2);
        assert_eq!(
            array[0],
            Value::new_uuid(
                Uuid::from_str("d7f4aeca-88f1-42a1-b385-b9db18abb255").unwrap(),
            )
        );
        assert_eq!(array[1], Value::new_uuid(Uuid::nil()));
    }

    #[test]
    fn read_dates() {
        let value = read_value_direct(
            "<llsd><array>
                 <date>2006-02-01T14:29:53Z</date>
                 <date>1999-01-01T00:00:00Z</date>
                 <date/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 3);
        assert_eq!(
            array[0],
            Value::new_date("2006-02-01T14:29:53Z".parse().unwrap())
        );
        assert_eq!(
            array[1],
            Value::new_date("1999-01-01T00:00:00Z".parse().unwrap())
        );
        assert_eq!(array[2], Value::new_date(NULL_DATE.clone()));
    }

    #[test]
    fn read_boolean() {
        let value = read_value_direct(
            "<llsd><array>
                 <boolean>1</boolean>
                 <boolean>true</boolean>
                 <boolean>0</boolean>
                 <boolean>false</boolean>
                 <boolean/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 5);
        assert_eq!(array[0], Value::new_boolean(true));
        assert_eq!(array[1], Value::new_boolean(true));
        assert_eq!(array[2], Value::new_boolean(false));
        assert_eq!(array[3], Value::new_boolean(false));
        assert_eq!(array[4], Value::new_boolean(false));
    }

    #[test]
    fn read_binary() {
        let value = read_value_direct(
            "<llsd><array>
                  <binary encoding='base64'>cmFuZG9t</binary>
                  <binary>dGhlIHF1aWNrIGJyb3duIGZveA==</binary>
                  <binary/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 3);
        assert_eq!(
            array[0],
            Value::new_binary(vec![114, 97, 110, 100, 111, 109])
        );
        assert_eq!(
            array[1],
            Value::new_binary(vec![
                116,
                104,
                101,
                32,
                113,
                117,
                105,
                99,
                107,
                32,
                98,
                114,
                111,
                119,
                110,
                32,
                102,
                111,
                120,
            ])
        );
        assert_eq!(array[2], Value::new_binary(Vec::new()));
    }

    #[test]
    fn read_undef() {
        let value = read_value_direct("<llsd><undef/></llsd>");
        assert_eq!(value, Value::Scalar(Scalar::Undefined));
    }

    #[test]
    fn read_uri() {
        let value = read_value_direct(
            "<llsd><array>
                 <uri>http://example.com:1000/list/files.xml</uri>
                 <uri/>
             </array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 2);
        assert_eq!(
            array[0],
            Value::new_uri("http://example.com:1000/list/files.xml")
        );
        assert_eq!(array[1], Value::new_uri(""));
    }

    #[test]
    fn read_array() {
        let value = read_value_direct(
            "<llsd><array><string>abc</string><integer>0</integer></array></llsd>",
        );
        let array = value.array().unwrap();

        assert_eq!(array.len(), 2);
        assert_eq!(array[0], Value::Scalar(Scalar::String("abc".to_string())));
        assert_eq!(array[1], Value::Scalar(Scalar::Integer(0)));
    }

    #[test]
    fn read_map() {
        let value = read_value_direct(
            "<llsd><map><key>a</key><integer>42</integer><key>b</key><integer>-42</integer></map></llsd>",
        );
        let map = value.map().unwrap();

        assert_eq!(map.len(), 2);
        assert_eq!(map["a"], Value::Scalar(Scalar::Integer(42)));
        assert_eq!(map["b"], Value::Scalar(Scalar::Integer(-42)));
    }

    #[test]
    fn extract_1() {
        let data = r#"<?xml version='1.0' encoding='UTF-8'?>
            <llsd>
                <map>
                    <key>region_id</key>
                    <uuid>67153d5b-3659-afb4-8510-adda2c034649</uuid>
                    <key>scale</key>
                    <string>one minute</string>
                    <key>simulator statistics</key>
                    <map>
                        <key>time dilation</key>
                        <real>0.9878624</real>
                        <key>sim fps</key>
                        <real>44.38898</real>
                        <key>agent updates per second</key>
                        <real>nan</real>
                        <key>total task count</key>
                        <real>4</real>
                        <key>active task count</key>
                        <real>0</real>
                        <key>pending uploads</key>
                        <real>0.0001096525</real>
                    </map>
                </map>
            </llsd>"#;

        let reader = Cursor::new(data);
        let value = read_value(reader).unwrap();

        let mut map = value.map().unwrap();

        assert_eq!(map.len(), 3);
        assert_eq!(
            map["region_id"],
            Value::Scalar(Scalar::Uuid(
                Uuid::from_str("67153d5b-3659-afb4-8510-adda2c034649")
                    .unwrap(),
            ))
        );
        assert_eq!(
            map["scale"],
            Value::Scalar(Scalar::String("one minute".to_string()))
        );

        let submap = map.remove("simulator statistics").unwrap().map().unwrap();
        assert_eq!(submap.len(), 6);
        assert_eq!(
            submap["time dilation"],
            Value::Scalar(Scalar::Real(0.9878624))
        );
        assert_eq!(submap["sim fps"], Value::Scalar(Scalar::Real(44.38898)));
        assert!(
            submap["agent updates per second"]
                .scalar_ref()
                .unwrap()
                .as_real()
                .unwrap()
                .is_nan()
        );
        assert_eq!(submap["total task count"], Value::Scalar(Scalar::Real(4.)));
        assert_eq!(submap["active task count"], Value::Scalar(Scalar::Real(0.)));
        assert_eq!(
            submap["pending uploads"],
            Value::Scalar(Scalar::Real(0.0001096525))
        );
    }
}
