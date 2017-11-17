//! The data types to be used.
pub use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDateTime, FixedOffset};
use byteorder::{BigEndian, ByteOrder};
use std::collections::HashMap;

pub type Date = DateTime<Utc>;

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Boolean(bool),
    Integer(i32),
    Real(f64),
    Uuid(Uuid),
    String(String),
    Date(Date),
    // TODO: Consider using a dedicated type here?
    Uri(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ScalarType {
    Boolean,
    Integer,
    Real,
    Uuid,
    String,
    Date,
    Uri,
    Binary
}

impl ScalarType {
    /// Parse a scalar from the specified data into a scalar with the variant
    /// specified by this Self instance.
    pub(crate) fn parse_scalar(&self, source: &[u8]) -> Option<Scalar> {
        // TODO: Consider optimizing so that only what is actually needed has to be
        // computed here, or will the compiler actually take care of this for us?
        let s_value = Scalar::String(String::from_utf8_lossy(source).to_string());
        let b_value = Scalar::Binary(source.to_vec());

        match *self {
            ScalarType::Boolean => s_value.as_bool().map(|b| Scalar::Boolean(b)),
            ScalarType::Integer => b_value.as_int().map(|i| Scalar::Integer(i)),
            ScalarType::Real => b_value.as_real().map(|r| Scalar::Real(r)),
            ScalarType::Uuid => b_value.as_uuid().map(|u| Scalar::Uuid(u)),
            ScalarType::String => Some(s_value),
            ScalarType::Date => s_value.as_date().map(|d| Scalar::Date(d)),
            ScalarType::Uri => s_value.as_uri().map(|u| Scalar::Uri(u)),
            ScalarType::Binary => Some(b_value),
        }
    }
}

pub type Map = HashMap<String, Value>;
pub type Array = Vec<Value>;

#[derive(Debug, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Map(Map),
    Array(Array),
    Undefined,
}

impl Scalar {
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Scalar::Boolean(ref b) => Some(*b),
            Scalar::Integer(ref i) => Some(*i != 0),
            Scalar::Real(ref r) => Some(*r != 0.),
            Scalar::Uuid(ref u) => Some(*u != Uuid::nil()),
            Scalar::String(ref s) => Some(!s.is_empty()),
            Scalar::Date(_) => None,
            Scalar::Uri(_) => None,
            Scalar::Binary(ref b) => Some(!b.is_empty()),
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match *self {
            Scalar::Boolean(ref b) => if *b { Some(1) } else { Some(0) },
            Scalar::Integer(ref i) => Some(*i),
            // Note: this can overflow, but never panics.
            Scalar::Real(ref r) => Some(r.round() as i32),
            Scalar::Uuid(_) => None,
            // TODO: "A simple conversion of the initial characters to an integer" ???
            Scalar::String(ref s) => unimplemented!(),
            Scalar::Date(ref d) => Some(d.timestamp() as i32),
            Scalar::Uri(_) => None,
            Scalar::Binary(ref b) => {
                if b.len() < 4 {
                    None
                } else {
                    Some(BigEndian::read_i32(&b[0..4]))
                }
            }
        }
    }

    pub fn as_real(&self) -> Option<f64> {
        match *self {
            Scalar::Boolean(ref b) => if *b { Some(1.) } else { Some(0.) },
            Scalar::Integer(ref i) => Some(*i as f64),
            Scalar::Real(ref r) => Some(*r),
            Scalar::Uuid(_) => None,
            // TODO:
            Scalar::String(ref s) => unimplemented!(),
            Scalar::Date(ref d) => Some(d.timestamp() as f64),
            Scalar::Uri(_) => None,
            Scalar::Binary(ref b) => {
                if b.len() < 8 {
                    None
                } else {
                    Some(BigEndian::read_f64(&b[0..8]))
                }
            }
        }
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match *self {
            Scalar::Boolean(_) => None,
            Scalar::Integer(_) => None,
            Scalar::Real(_) => None,
            Scalar::Uuid(ref u) => Some(u.clone()),
            // TODO: This doesn't correctly implement the spec, as the spec says only the
            // conversion of hyphenated UUIDs should succeed, every other should fail,
            // but this method is agnostic of the hyphens.
            Scalar::String(ref s) => Uuid::parse_str(s.as_str()).ok(),
            Scalar::Date(_) => None,
            Scalar::Uri(_) => None,
            Scalar::Binary(ref b) => {
                if b.len() < 16 {
                    None
                } else {
                    // We could even unwrap, but just in case they add more error causes in the future,
                    // this is the safest way.
                    Uuid::from_bytes(&b[0..16]).ok()
                }
            }
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match *self {
            Scalar::Boolean(ref b) => {
                if *b {
                    Some("true".to_string())
                } else {
                    Some("false".to_string())
                }
            }
            Scalar::Integer(ref i) => Some(format!("{}", i)),
            Scalar::Real(ref r) => Some(format!("{}", r)),
            Scalar::Uuid(ref u) => Some(u.hyphenated().to_string()),
            Scalar::String(ref s) => Some(s.clone()),
            Scalar::Date(ref d) => Some(d.to_rfc3339()),
            Scalar::Uri(ref u) => Some(u.clone()),
            Scalar::Binary(ref b) => Some(String::from_utf8_lossy(b).to_string()),
        }
    }

    pub fn as_date(&self) -> Option<Date> {
        match *self {
            Scalar::Boolean(_) => None,
            // TODO: I can't imagine anyone ever wants to use this with a i32, maybe this is
            // another error in the documentation?
            Scalar::Integer(ref i) => Some(Date::from_utc(
                NaiveDateTime::from_timestamp(*i as i64, 0),
                Utc,
            )),
            Scalar::Real(ref f) => Some(Date::from_utc(
                NaiveDateTime::from_timestamp(*f as i64, 0),
                Utc,
            )),
            Scalar::Uuid(_) => None,
            Scalar::String(ref s) => s.parse().ok(),
            Scalar::Date(ref d) => Some(d.clone()),
            Scalar::Uri(_) => None,
            Scalar::Binary(_) => None,
        }
    }

    pub fn as_uri(&self) -> Option<String> {
        match *self {
            Scalar::Boolean(_) => None,
            Scalar::Integer(_) => None,
            Scalar::Real(_) => None,
            Scalar::Uuid(_) => None,
            Scalar::String(ref s) => Some(s.clone()),
            Scalar::Date(_) => None,
            Scalar::Uri(ref u) => Some(u.clone()),
            Scalar::Binary(ref b) => Some(String::from_utf8_lossy(b).to_string()),
        }
    }

    pub fn as_binary(&self) -> Option<Vec<u8>> {
        match *self {
            Scalar::Boolean(ref b) => if *b { Some(vec![1]) } else { Some(vec![0]) },
            Scalar::Integer(ref i) => {
                let mut buf = Vec::new();
                BigEndian::write_i32(&mut buf, *i);
                Some(buf)
            }
            Scalar::Real(ref r) => {
                let mut buf = Vec::new();
                BigEndian::write_f64(&mut buf, *r);
                Some(buf)
            }
            Scalar::Uuid(ref u) => Some(u.as_bytes().to_vec()),
            Scalar::String(ref s) => Some(s.as_bytes().to_vec()),
            Scalar::Date(_) => None,
            Scalar::Uri(_) => None,
            Scalar::Binary(ref b) => Some(b.clone()),
        }
    }
}
