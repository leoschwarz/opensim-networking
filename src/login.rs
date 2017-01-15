use crypto::md5::Md5;
use crypto::digest::Digest;
use std::collections::BTreeMap;
use xmlrpc::Value as XmlValue;
use nalgebra::Vector3;
use regex::Regex;
use Ip4Addr;
use std::str::FromStr;
use Uuid;

/// Performing a LoginRequest is the first step at gaining access to a sim.
#[derive(Debug)]
pub struct LoginRequest {
    /// User first name
    pub first_name: String,
    /// User last name
    pub last_name: String,
    /// Hashed password
    pub password_hash: String,
    /// Start location ("last", "home", or: named region and location)
    pub start: String,
}

/// Hash a plain text password into the format required for login.
pub fn hash_password(password_raw: &str) -> String {
    // Hash the password.
    let mut digest = Md5::new();
    digest.input_str(password_raw);

    // Bring into required format.
    "$1$".to_string() + &digest.result_str()
}

#[derive(Debug)]
pub enum LoginError {
    /// There was a HTTP error.
    HyperError(::hyper::error::Error),
    /// There was an error with parsing the response.
    ParserError,
    /// The server returned an explicit failure as response.
    Fail,
    /// The server returned an invalid XML-RPC response.
    InvalidResponse
}

impl From<::xmlrpc::RequestError> for LoginError {
    fn from(err: ::xmlrpc::RequestError) -> LoginError {
        println!("error: {:?}", err);
        match err {
            ::xmlrpc::RequestError::HyperError(e) => LoginError::HyperError(e),
            ::xmlrpc::RequestError::ParseError(_) => LoginError::ParserError,
        }
    }
}

impl From<::std::num::ParseFloatError> for LoginError {
    fn from(_: ::std::num::ParseFloatError) -> LoginError {
        LoginError::InvalidResponse
    }
}

impl From<::std::net::AddrParseError> for LoginError {
    fn from(_: ::std::net::AddrParseError) -> LoginError {
        LoginError::InvalidResponse
    }
}

impl From<::uuid::ParseError> for LoginError {
    fn from(_: ::uuid::ParseError) -> LoginError {
        LoginError::InvalidResponse
    }
}

#[derive(Debug)]
pub struct LoginResponse {
    pub look_at: Vector3<f32>,
    pub circuit_code: u32,
    pub session_id: Uuid,
    pub agent_id: Uuid,

    /// The IP address of the simulator to connect to.
    pub sim_ip: Ip4Addr,
    /// The port of the simulator to connect to.
    pub sim_port: u16
}

impl LoginResponse {
    fn extract_vector3(raw: &str) -> Result<Vector3<f32>, LoginError> {
        let re = Regex::new(r"\[r([0-9\.-]+),r([0-9\.-]+),r([0-9\.-]+)\]").unwrap();
        match re.captures(raw) {
            Some(caps) => {
                let x = try!(caps.at(1).unwrap().parse::<f32>());
                let y = try!(caps.at(2).unwrap().parse::<f32>());
                let z = try!(caps.at(3).unwrap().parse::<f32>());
                Ok(Vector3::new(x,y,z))
            },
            _ => Err(LoginError::InvalidResponse)
        }
    }

    fn extract(response: BTreeMap<String, XmlValue>) -> Result<LoginResponse, LoginError> {
        // TODO: Check if additional items should be extracted.
        let look_at = match response.get("look_at") {
            Some(&XmlValue::String(ref raw)) => try!(LoginResponse::extract_vector3(raw)),
            _ => return Err(LoginError::InvalidResponse)
        };
        let circuit_code = match response.get("circuit_code") {
            Some(&XmlValue::Int(code)) => code as u32,
            _ => return Err(LoginError::InvalidResponse)
        };
        let session_id = match response.get("session_id") {
            Some(&XmlValue::String(ref id)) => try!(Uuid::parse_str(id)),
            _ => return Err(LoginError::InvalidResponse)
        };
        let agent_id = match response.get("agent_id") {
            Some(&XmlValue::String(ref id)) => try!(Uuid::parse_str(id)),
            _ => return Err(LoginError::InvalidResponse)
        };
        let sim_ip = match response.get("sim_ip") {
            Some(&XmlValue::String(ref ip_raw)) => try!(Ip4Addr::from_str(ip_raw)),
            _ => return Err(LoginError::InvalidResponse)
        };
        let sim_port = match response.get("sim_port") {
            Some(&XmlValue::Int(port)) => port as u16,
            _ => return Err(LoginError::InvalidResponse)
        };

        Ok(LoginResponse {
            look_at: look_at,
            circuit_code: circuit_code,
            session_id: session_id,
            agent_id: agent_id,
            sim_ip: sim_ip,
            sim_port: sim_port
        })
    }
}

#[test]
fn test_extract_vector3() {
    // Test correct behavior.
    let result = LoginResponse::extract_vector3("[r0.171732,r0.9851437,r0]").unwrap();
    let eps = 0.00001;
    assert!((result.x - 0.171732).abs() < eps);
    assert!((result.y - 0.9851437).abs() < eps);
    assert!((result.z - 0.).abs() < eps);

    // Test graceful failure.
    assert!(LoginResponse::extract_vector3("Lorem ipsum").is_err());
}

impl LoginRequest {
    pub fn perform(&self, url: &str) -> Result<LoginResponse, LoginError> {
        let mut data: BTreeMap<String, XmlValue> = BTreeMap::new();
        data.insert("first".to_string(), XmlValue::from(&self.first_name[..]));
        data.insert("last".to_string(), XmlValue::from(&self.last_name[..]));
        data.insert("passwd".to_string(), XmlValue::from(&self.password_hash[..]));
        data.insert("start".to_string(), XmlValue::from(&self.start[..]));
        data.insert("version".to_string(), XmlValue::from("0.1.0"));
        data.insert("channel".to_string(), XmlValue::from("tokio-opensim"));
        data.insert("platform".to_string(), XmlValue::from("Linux"));

        let client = ::hyper::Client::new();

        let result = try!(::xmlrpc::Request::new("login_to_simulator")
            .arg(XmlValue::Struct(data)).call(&client, url));

        match result {
            Err(_) => Err(LoginError::Fail),
            Ok(response) => match response {
                XmlValue::Struct(s) => LoginResponse::extract(s),
                _ => Err(LoginError::InvalidResponse)
            }
        }
    }
}

