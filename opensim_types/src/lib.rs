// TODO: This crate should be renamed.

pub extern crate nalgebra;
extern crate url;
extern crate uuid;

pub use nalgebra::{DMatrix, Matrix, Matrix3, Matrix4, MatrixN, Quaternion, UnitQuaternion, Vector2,
                   Vector3, Vector4};
pub use std::net::Ipv4Addr as Ip4Addr;
pub use std::net::IpAddr;
pub use std::time::{Duration, Instant};

use nalgebra::MatrixVec;
use nalgebra::core::dimension::{U256, U512};
pub type Matrix256<S> = Matrix<S, U256, U256, MatrixVec<S, U256, U256>>;
pub type Matrix512<S> = Matrix<S, U512, U512, MatrixVec<S, U512, U512>>;

pub type IpPort = u16;

pub use uuid::Uuid;
pub use uuid::ParseError as UuidParseError;

pub use url::ParseError as UrlParseError;

pub type SequenceNumber = u32;
