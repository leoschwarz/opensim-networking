pub use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDateTime, FixedOffset};
use byteorder::{BigEndian, ByteOrder};

pub type Date = DateTime<Utc>;

#[derive(Debug)]
pub enum ScalarType {
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

impl ScalarType {
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            ScalarType::Boolean(ref b) => Some(*b),
            ScalarType::Integer(ref i) => Some(*i != 0),
            ScalarType::Real(ref r) => Some(*r != 0.),
            ScalarType::Uuid(ref u) => Some(*u != Uuid::nil()),
            ScalarType::String(ref s) => Some(!s.is_empty()),
            ScalarType::Date(_) => None,
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => Some(!b.is_empty()),
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match *self {
            ScalarType::Boolean(ref b) => if *b { Some(1) } else { Some(0) },
            ScalarType::Integer(ref i) => Some(*i),
            // Note: this can overflow, but never panics.
            ScalarType::Real(ref r) => Some(r.round() as i32),
            ScalarType::Uuid(_) => None,
            // TODO: "A simple conversion of the initial characters to an integer" ???
            ScalarType::String(ref s) => unimplemented!(),
            ScalarType::Date(ref d) => Some(d.timestamp() as i32),
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => {
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
            ScalarType::Boolean(ref b) => if *b { Some(1.) } else { Some(0.) },
            ScalarType::Integer(ref i) => Some(*i as f64),
            ScalarType::Real(ref r) => Some(*r),
            ScalarType::Uuid(_) => None,
            // TODO:
            ScalarType::String(ref s) => unimplemented!(),
            ScalarType::Date(ref d) => Some(d.timestamp() as f64),
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => {
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
            ScalarType::Boolean(_) => None,
            ScalarType::Integer(_) => None,
            ScalarType::Real(_) => None,
            ScalarType::Uuid(ref u) => Some(u.clone()),
            // TODO: This doesn't correctly implement the spec, as the spec says only the
            // conversion of hyphenated UUIDs should succeed, every other should fail,
            // but this method is agnostic of the hyphens.
            ScalarType::String(ref s) => Uuid::parse_str(s.as_str()).ok(),
            ScalarType::Date(_) => None,
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => {
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
            ScalarType::Boolean(ref b) => {
                if *b {
                    Some("true".to_string())
                } else {
                    Some("false".to_string())
                }
            }
            ScalarType::Integer(ref i) => Some(format!("{}", i)),
            ScalarType::Real(ref r) => Some(format!("{}", r)),
            ScalarType::Uuid(ref u) => Some(u.hyphenated().to_string()),
            ScalarType::String(ref s) => Some(s.clone()),
            ScalarType::Date(ref d) => Some(d.to_rfc3339()),
            ScalarType::Uri(ref u) => Some(u.clone()),
            ScalarType::Binary(ref b) => Some(String::from_utf8_lossy(b).to_string()),
        }
    }

    pub fn as_date(&self) -> Option<Date> {
        match *self {
            ScalarType::Boolean(_) => None,
            // TODO: I can't imagine anyone ever wants to use this with a i32, maybe this is
            // another error in the documentation?
            ScalarType::Integer(ref i) => Some(Date::from_utc(
                NaiveDateTime::from_timestamp(*i as i64, 0),
                Utc,
            )),
            ScalarType::Real(ref f) => Some(Date::from_utc(
                NaiveDateTime::from_timestamp(*f as i64, 0),
                Utc,
            )),
            ScalarType::Uuid(_) => None,
            ScalarType::String(ref s) => s.parse().ok(),
            ScalarType::Date(ref d) => Some(d.clone()),
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => None,
        }
    }

    pub fn as_uri(&self) -> Option<String> {
        match *self {
            ScalarType::Boolean(_) => None,
            ScalarType::Integer(_) => None,
            ScalarType::Real(_) => None,
            ScalarType::Uuid(_) => None,
            ScalarType::String(ref s) => Some(s.clone()),
            ScalarType::Date(_) => None,
            ScalarType::Uri(ref u) => Some(u.clone()),
            ScalarType::Binary(ref b) => Some(String::from_utf8_lossy(b).to_string()),
        }
    }

    pub fn as_binary(&self) -> Option<Vec<u8>> {
        match *self {
            ScalarType::Boolean(ref b) => if *b {Some(vec![1])} else { Some(vec![0])} ,
            ScalarType::Integer(ref i) => {
                let mut buf = Vec::new();
                BigEndian::write_i32(&mut buf, *i);
                Some(buf)
            },
            ScalarType::Real(ref r) => {
                let mut buf = Vec::new();
                BigEndian::write_f64(&mut buf, *r);
                Some(buf)
            },
            ScalarType::Uuid(ref u) => Some(u.as_bytes().to_vec()),
            ScalarType::String(ref s) => Some(s.as_bytes().to_vec()),
            ScalarType::Date(_) => None,
            ScalarType::Uri(_) => None,
            ScalarType::Binary(ref b) => Some(b.clone()),
        }
    }
}
