//! Handling for the binary representation of LLSD data.

// TODO check in the right place for the header "<? LLSD/Binary ?>\n" and remove it from the
// reader.

// TODO: if needed also writer

use data::*;
use std::io::Read;
use byteorder::{ByteOrder, BigEndian, ReadBytesExt};

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ReadError")]
#[error_chain(result = "")]
pub enum ReadErrorKind {
    #[error_chain(foreign)]
    Io(::std::io::Error),

    #[error_chain(custom)]
    InvalidKey,

    #[error_chain(custom)]
    InvalidTypePrefix,
}

fn read_n_bytes<R: Read>(reader: &mut R, n_bytes: u32) -> Result<Vec<u8>, ReadError> {
    let mut data = vec![0u8; n_bytes as usize];
    reader.read_exact(&mut data)?;
    Ok(data.to_vec())
}

pub fn read_value<R: Read>(reader: &mut R) -> Result<Value, ReadError> {
    let code = reader.read_u8()? as char;
    match code {
        '!' => Ok(Value::Undefined),
        '1' => Ok(Value::Scalar(Scalar::Boolean(true))),
        '0' => Ok(Value::Scalar(Scalar::Boolean(false))),
        'i' => Ok(Value::Scalar(
            Scalar::Integer(reader.read_i32::<BigEndian>()?),
        )),
        'r' => Ok(Value::Scalar(Scalar::Real(reader.read_f64::<BigEndian>()?))),
        'u' => Ok(Value::Scalar(Scalar::Uuid(unimplemented!()))),
        'b' => {
            let len = reader.read_u32::<BigEndian>()?;
            Ok(Value::Scalar(Scalar::Binary(read_n_bytes(reader, len)?)))
        }
        's' => {
            let len = reader.read_u32::<BigEndian>()?;
            let data = read_n_bytes(reader, len)?;
            Ok(Value::Scalar(
                Scalar::String(String::from_utf8_lossy(&data).to_string()),
            ))
        }
        'l' => {
            let len = reader.read_u32::<BigEndian>()?;
            let data = read_n_bytes(reader, len)?;
            Ok(Value::Scalar(
                Scalar::Uri(String::from_utf8_lossy(&data[..]).to_string()),
            ))
        }
        'd' => {
            let real = Scalar::Real(reader.read_f64::<BigEndian>()?);
            Ok(Value::Scalar(Scalar::Date(real.as_date().unwrap())))
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
