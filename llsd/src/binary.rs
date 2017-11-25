//! Handle binary representation of LLSD data.

use data::*;
use std::io::Read;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};

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

// This assumes that the header has already been skipped by the initial caller.
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
            0x75, 0x97, 0xf4, 0xae, 0xca, 0x88, 0xa1, 0x42, 0xa1, 0xb3, 0x85, 0xb9, 0x7b, 0x18, 0xab, 0xb2,
            0x55,
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
            0x62, 0x0, 0x0, 0x0, 0x34, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, 0x20, 0x61, 0x20, 0x73,
            0x69, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x62, 0x69, 0x6e, 0x61, 0x72, 0x79, 0x20, 0x63, 0x6f, 0x6e,
            0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x20, 0x66, 0x6f, 0x72, 0x20, 0x74, 0x68, 0x69, 0x73,
            0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0xa, 0xd,
        ];

        assert_eq!(
            read_unwrap(data),
            Value::new_binary(vec![
                0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, 0x20, 0x61, 0x20, 0x73, 0x69, 0x6d, 0x70, 0x6c,
                0x65, 0x20, 0x62, 0x69, 0x6e, 0x61, 0x72, 0x79, 0x20, 0x63, 0x6f, 0x6e, 0x76, 0x65, 0x72,
                0x73, 0x69, 0x6f, 0x6e, 0x20, 0x66, 0x6f, 0x72, 0x20, 0x74, 0x68, 0x69, 0x73, 0x20, 0x73,
                0x74, 0x72, 0x69, 0x6e, 0x67, 0xa, 0xd,
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
            0x73, 0x0, 0x0, 0x0, 0x25, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b,
            0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x30,
            0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x30,
        ];
        assert_eq!(
            read_unwrap(data),
            Value::new_string("abcdefghijklmnopqrstuvwxyz01234567890")
        );
    }

    #[test]
    fn read_uri() {
        let data = vec![
            0x6c, 0x0, 0x0, 0x0, 0x18, 0x68, 0x74, 0x74, 0x70, 0x3a, 0x2f, 0x2f, 0x77, 0x77, 0x77, 0x2e,
            0x74, 0x65, 0x73, 0x74, 0x75, 0x72, 0x6c, 0x2e, 0x74, 0x65, 0x73, 0x74, 0x2f,
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
            0x7b, 0x0, 0x0, 0x0, 0x1, 0x6b, 0x0, 0x0, 0x0, 0x4, 0x74, 0x65, 0x73, 0x74, 0x69, 0x0, 0x0, 0x0,
            0x0, 0x7d,
        ];
        let map = read_unwrap(data).map().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map["test"], Value::new_integer(0));

        // { t0st = 241, tes1 = "aha", test = undef }
        let data = vec![
            0x7b, 0x0, 0x0, 0x0, 0x3, 0x6b, 0x0, 0x0, 0x0, 0x4, 0x74, 0x65, 0x73, 0x74, 0x21, 0x6b, 0x0, 0x0,
            0x0, 0x4, 0x74, 0x65, 0x73, 0x31, 0x73, 0x0, 0x0, 0x0, 0x3, 0x61, 0x68, 0x61, 0x6b, 0x0, 0x0,
            0x0, 0x4, 0x74, 0x30, 0x73, 0x74, 0x69, 0x0, 0x0, 0x0, 0xf1, 0x7d,
        ];
        let map = read_unwrap(data).map().unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map["t0st"], Value::new_integer(241));
        assert_eq!(map["tes1"], Value::new_string("aha"));
        assert_eq!(map["test"], Value::Scalar(Scalar::Undefined));
    }
}
