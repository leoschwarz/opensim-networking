//! Handle binary representation of LLSD data.

use data::*;
use std::io::{Read, Write};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ReadError")]
#[error_chain(result = "")]
pub enum ReadErrorKind {
    #[error_chain(foreign)] Io(::std::io::Error),

    #[error_chain(foreign)] Uuid(::uuid::ParseError),

    #[error_chain(custom)] InvalidKey,

    #[error_chain(custom)] InvalidTypePrefix,
}

fn read_n_bytes<R: Read>(reader: &mut R, n_bytes: u32) -> Result<Vec<u8>, ReadError> {
    let mut data = vec![0u8; n_bytes as usize];
    reader.read_exact(&mut data)?;
    Ok(data.to_vec())
}

/// Read an LLSD value from its binary representation.
///
/// This assumes that the header has already been skipped by the initial caller.
pub fn read_value<R: Read>(reader: &mut R) -> Result<Value, ReadError> {
    let code = reader.read_u8()? as char;
    match code {
        '!' => Ok(Value::Scalar(Scalar::Undefined)),
        '1' => Ok(Value::new_boolean(true)),
        '0' => Ok(Value::new_boolean(false)),
        'i' => Ok(Value::new_integer(reader.read_i32::<BigEndian>()?)),
        'r' => Ok(Value::new_real(reader.read_f64::<BigEndian>()?)),
        'u' => {
            let mut bytes = [0u8; 16];
            reader.read_exact(&mut bytes)?;
            Ok(Value::new_uuid(Uuid::from_bytes(&bytes)?))
        }
        'b' => {
            let len = reader.read_u32::<BigEndian>()?;
            Ok(Value::new_binary(read_n_bytes(reader, len)?))
        }
        's' | 'k' => {
            let len = reader.read_u32::<BigEndian>()?;
            let data = read_n_bytes(reader, len)?;
            Ok(Value::new_string(String::from_utf8_lossy(&data)))
        }
        'l' => {
            let len = reader.read_u32::<BigEndian>()?;
            let data = read_n_bytes(reader, len)?;
            Ok(Value::new_uri(String::from_utf8_lossy(&data)))
        }
        'd' => {
            let real = Scalar::Real(reader.read_f64::<LittleEndian>()?);
            Ok(Value::new_date(real.as_date().unwrap()))
        }
        '[' => {
            let len = reader.read_u32::<BigEndian>()?;
            let mut items = Vec::new();

            for _ in 0..len {
                items.push(read_value(reader)?);
            }

            // ']'
            reader.read_u8()?;
            Ok(Value::Array(items))
        }
        '{' => {
            let len = reader.read_u32::<BigEndian>()?;
            let mut items = Map::new();

            for _ in 0..len {
                let key = match read_value(reader)? {
                    Value::Scalar(Scalar::String(s)) => s,
                    _ => return Err(ReadErrorKind::InvalidKey.into()),
                };
                let value = read_value(reader)?;
                items.insert(key, value);
            }

            // ']'
            reader.read_u8()?;
            Ok(Value::Map(items))
        }
        _ => Err(ReadErrorKind::InvalidTypePrefix.into()),
    }
}

/// Writes a Value to the writer.
///
/// Note that this does not write the LLSD header.
pub fn write_value<W: Write>(writer: &mut W, value: &Value) -> Result<(), ::std::io::Error> {
    match *value {
        Value::Scalar(Scalar::Boolean(ref b)) => {
            writer.write_u8(if *b { '1' as u8 } else { '0' as u8 })
        }
        Value::Scalar(Scalar::Integer(ref i)) => {
            writer.write_u8('i' as u8)?;
            writer.write_i32::<BigEndian>(*i)
        }
        Value::Scalar(Scalar::Real(ref r)) => {
            writer.write_u8('r' as u8)?;
            writer.write_f64::<BigEndian>(*r)
        }
        Value::Scalar(Scalar::Uuid(ref u)) => {
            writer.write_u8('u' as u8)?;
            writer.write_all(u.as_bytes())
        }
        Value::Scalar(Scalar::String(ref s)) => {
            writer.write_u8('s' as u8)?;
            let bytes = s.as_bytes();
            writer.write_u32::<BigEndian>(bytes.len() as u32)?;
            writer.write_all(bytes)
        }
        Value::Scalar(Scalar::Date(ref d)) => {
            writer.write_u8('d' as u8)?;
            // TODO
            let date = Scalar::Date(d.clone());
            writer.write_f64::<LittleEndian>(date.as_real().unwrap())
        }
        Value::Scalar(Scalar::Uri(ref u)) => {
            writer.write_u8('l' as u8)?;
            let bytes = u.as_bytes();
            writer.write_u32::<BigEndian>(bytes.len() as u32)?;
            writer.write_all(bytes)
        }
        Value::Scalar(Scalar::Binary(ref b)) => {
            writer.write_u8('b' as u8)?;
            writer.write_u32::<BigEndian>(b.len() as u32)?;
            writer.write_all(b)
        }
        Value::Scalar(Scalar::Undefined) => writer.write_u8('!' as u8),
        Value::Map(ref map) => {
            writer.write_u8('{' as u8)?;
            writer.write_u32::<BigEndian>(map.len() as u32)?;
            for (key, val) in map {
                // Key
                writer.write_u8('k' as u8)?;
                let bytes = key.as_bytes();
                writer.write_u32::<BigEndian>(bytes.len() as u32)?;
                writer.write_all(bytes)?;

                // Value
                write_value(writer, val)?;
            }
            writer.write_u8('}' as u8)
        }
        Value::Array(ref arr) => {
            writer.write_u8('[' as u8)?;
            writer.write_u32::<BigEndian>(arr.len() as u32)?;
            for item in arr {
                write_value(writer, item)?;
            }
            writer.write_u8(']' as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    // test helper
    fn read_unwrap(bytes: Vec<u8>) -> Value {
        let mut reader = BufReader::new(&bytes[..]);
        read_value(&mut reader).unwrap()
    }

    #[test]
    fn read_undef() {
        let value = read_unwrap(vec![0x21]);
        assert_eq!(value, Value::Scalar(Scalar::Undefined));
    }

    #[test]
    fn read_boolean() {
        assert_eq!(read_unwrap(vec![0x31]), Value::new_boolean(true));
        assert_eq!(read_unwrap(vec![0x30]), Value::new_boolean(false));
    }

    #[test]
    fn read_integer() {
        assert_eq!(
            read_unwrap(vec![
                0x69, 0x0, 0x0, 0x0, 0x0
            ]),
            Value::new_integer(0)
        );
        assert_eq!(
            read_unwrap(vec![
                0x69, 0x0, 0x12, 0xd7, 0x9b
            ]),
            Value::new_integer(1234843)
        );
    }

    #[test]
    fn read_real() {
        let data = vec![
            0x72, 0x41, 0x2c, 0xec, 0xf6, 0x77, 0xce, 0xd9, 0x17
        ];
        assert_eq!(read_unwrap(data), Value::new_real(947835.234));
    }

    #[test]
    fn read_uuid() {
        let data = vec![
            0x75, 0x97, 0xf4, 0xae, 0xca, 0x88, 0xa1, 0x42, 0xa1, 0xb3, 0x85, 0xb9, 0x7b, 0x18,
            0xab, 0xb2, 0x55,
        ];
        assert_eq!(
            read_unwrap(data),
            Value::new_uuid("97f4aeca-88a1-42a1-b385-b97b18abb255".parse().unwrap())
        );

        let data = vec![
            0x75, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ];
        assert_eq!(
            read_unwrap(data),
            Value::new_uuid("00000000-0000-0000-0000-000000000000".parse().unwrap())
        );
    }

    #[test]
    fn read_binary() {
        let data = vec![
            0x62, 0x0, 0x0, 0x0, 0x34, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, 0x20, 0x61, 0x20,
            0x73, 0x69, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x62, 0x69, 0x6e, 0x61, 0x72, 0x79, 0x20,
            0x63, 0x6f, 0x6e, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x20, 0x66, 0x6f, 0x72,
            0x20, 0x74, 0x68, 0x69, 0x73, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0xa, 0xd,
        ];

        assert_eq!(
            read_unwrap(data),
            Value::new_binary(vec![
                0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, 0x20, 0x61, 0x20, 0x73, 0x69, 0x6d, 0x70,
                0x6c, 0x65, 0x20, 0x62, 0x69, 0x6e, 0x61, 0x72, 0x79, 0x20, 0x63, 0x6f, 0x6e, 0x76,
                0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x20, 0x66, 0x6f, 0x72, 0x20, 0x74, 0x68, 0x69,
                0x73, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0xa, 0xd,
            ])
        );
    }

    #[test]
    fn read_string() {
        let data = vec![
            0x73, 0, 0, 0, 0
        ];
        assert_eq!(read_unwrap(data), Value::new_string(""));

        let data = vec![
            0x73, 0x0, 0x0, 0x0, 0x25, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a,
            0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
            0x79, 0x7a, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x30,
        ];
        assert_eq!(
            read_unwrap(data),
            Value::new_string("abcdefghijklmnopqrstuvwxyz01234567890")
        );
    }

    #[test]
    fn read_uri() {
        let data = vec![
            0x6c, 0x0, 0x0, 0x0, 0x18, 0x68, 0x74, 0x74, 0x70, 0x3a, 0x2f, 0x2f, 0x77, 0x77, 0x77,
            0x2e, 0x74, 0x65, 0x73, 0x74, 0x75, 0x72, 0x6c, 0x2e, 0x74, 0x65, 0x73, 0x74, 0x2f,
        ];
        assert_eq!(
            read_unwrap(data),
            Value::new_uri("http://www.testurl.test/")
        );
    }

    #[test]
    fn read_datetime() {
        use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};

        let data = vec![
            100, 0, 0, 192, 141, 167, 222, 209, 65
        ];
        let d = NaiveDate::from_ymd(2008, 1, 1);
        let t = NaiveTime::from_hms_milli(20, 10, 31, 0);
        let date = Date::from_utc(NaiveDateTime::new(d, t), Utc);
        assert_eq!(read_unwrap(data), Value::new_date(date));
    }

    #[test]
    fn read_array() {
        // Empty array.
        let data = vec![
            0x5b, 0x0, 0x0, 0x0, 0x0, 0x5d
        ];
        assert_eq!(read_unwrap(data), Value::Array(Vec::new()));

        // { 0 }
        let data = vec![
            0x5b, 0x0, 0x0, 0x0, 0x1, 0x69, 0x0, 0x0, 0x0, 0x0, 0x5d
        ];
        let arr = read_unwrap(data).array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], Value::new_integer(0));

        // { 0, 0 }
        let data = vec![
            0x5b, 0x0, 0x0, 0x0, 0x2, 0x69, 0x0, 0x0, 0x0, 0x0, 0x69, 0x0, 0x0, 0x0, 0x0, 0x5d
        ];
        let arr = read_unwrap(data).array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::new_integer(0));
        assert_eq!(arr[1], Value::new_integer(0));
    }

    #[test]
    fn read_map() {
        // {}
        let data = vec![
            0x7b, 0x0, 0x0, 0x0, 0x0, 0x7d
        ];
        assert_eq!(read_unwrap(data), Value::Map(Map::new()));

        // { test = 0 }
        let data = vec![
            0x7b, 0x0, 0x0, 0x0, 0x1, 0x6b, 0x0, 0x0, 0x0, 0x4, 0x74, 0x65, 0x73, 0x74, 0x69, 0x0,
            0x0, 0x0, 0x0, 0x7d,
        ];
        let map = read_unwrap(data).map().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map["test"], Value::new_integer(0));

        // { t0st = 241, tes1 = "aha", test = undef }
        let data = vec![
            0x7b, 0x0, 0x0, 0x0, 0x3, 0x6b, 0x0, 0x0, 0x0, 0x4, 0x74, 0x65, 0x73, 0x74, 0x21, 0x6b,
            0x0, 0x0, 0x0, 0x4, 0x74, 0x65, 0x73, 0x31, 0x73, 0x0, 0x0, 0x0, 0x3, 0x61, 0x68, 0x61,
            0x6b, 0x0, 0x0, 0x0, 0x4, 0x74, 0x30, 0x73, 0x74, 0x69, 0x0, 0x0, 0x0, 0xf1, 0x7d,
        ];
        let map = read_unwrap(data).map().unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map["t0st"], Value::new_integer(241));
        assert_eq!(map["tes1"], Value::new_string("aha"));
        assert_eq!(map["test"], Value::Scalar(Scalar::Undefined));
    }

    #[test]
    fn write() {
        use std::collections::HashMap;
        use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};

        let mut map = HashMap::new();
        map.insert("bool_0".to_string(), Value::new_boolean(false));
        map.insert("bool_1".to_string(), Value::new_boolean(true));
        map.insert("int".to_string(), Value::new_integer(42));
        map.insert("real".to_string(), Value::new_real(1.2141e30));
        map.insert(
            "uuid".to_string(),
            Value::new_uuid("7ad22c95-f7c2-47ab-9525-ca64135c928c".parse().unwrap()),
        );
        map.insert("string".to_string(), Value::new_string("Lorem ipsum"));
        let d = NaiveDate::from_ymd(2008, 1, 1);
        let t = NaiveTime::from_hms_milli(20, 10, 31, 0);
        let date = Date::from_utc(NaiveDateTime::new(d, t), Utc);
        map.insert("date".to_string(), Value::new_date(date));
        map.insert("uri".to_string(), Value::new_uri("http://example.com"));
        map.insert(
            "binary".to_string(),
            Value::new_binary(vec![
                10, 11, 12, 13, 5, 6, 7, 8
            ]),
        );
        map.insert("undef".to_string(), Value::Scalar(Scalar::Undefined));
        map.insert(
            "arr".to_string(),
            Value::Array(vec![
                Value::new_string("abc"),
                Value::new_string("xyz"),
                Value::new_real(123.456),
            ]),
        );
        let data_in = Value::Map(map);

        let mut ser = Vec::new();
        write_value(&mut ser, &data_in).unwrap();

        let data_out = read_unwrap(ser);

        assert_eq!(data_out, data_in);
    }
}
