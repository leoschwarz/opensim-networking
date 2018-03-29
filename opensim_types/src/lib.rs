// TODO: This crate should be renamed.

pub extern crate nalgebra;
extern crate url;
extern crate uuid;

pub use nalgebra::{DMatrix, Matrix, Matrix3, Matrix4, Quaternion, UnitQuaternion, Vector2,
                   Vector3, Vector4};
pub use std::net::IpAddr;
pub use std::net::Ipv4Addr as Ip4Addr;
pub use std::time::{Duration, Instant};

use nalgebra::core::dimension::{U16, U32};
use nalgebra::{MatrixArray, MatrixVec};

/// Stack allocated square matrix.
pub type MatrixSN<S, N> = Matrix<S, N, N, MatrixArray<S, N, N>>;

/// Heap allocated square matrix.
pub type MatrixHN<S, N> = Matrix<S, N, N, MatrixVec<S, N, N>>;

pub type Matrix16<S> = Matrix<S, U16, U16, MatrixVec<S, U16, U16>>;
pub type Matrix32<S> = Matrix<S, U32, U32, MatrixVec<S, U32, U32>>;

pub type IpPort = u16;

pub use uuid::ParseError as UuidParseError;
pub use uuid::Uuid;

pub use url::ParseError as UrlParseError;

pub type SequenceNumber = u32;
