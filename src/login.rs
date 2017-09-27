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

#[derive(Debug, ErrorChain)]
#[error_chain(error = "LoginError")]
#[error_chain(result = "")]
pub enum LoginErrorKind {
    #[error_chain(foreign)]
    RequestError(::xmlrpc::RequestError),

    #[error_chain(foreign)]
    HttpError(::reqwest::Error),

    #[error_chain(foreign)]
    ParseFloatError(::std::num::ParseFloatError),

    #[error_chain(foreign)]
    AddrParseError(::std::net::AddrParseError),

    #[error_chain(foreign)]
    UuidParseError(::uuid::ParseError),

    /// Login failed.
    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "login failed""#)]
    #[error_chain(display = r#"|fault| write!(f, "login failed: {:?}", fault)"#)]
    XmlRpcFault(::xmlrpc::Fault),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "extracting response failed""#)]
    #[error_chain(display = r#"|field| write!(f, "extracting {} from response failed", field)"#)]
    ExtractResponseError(String),
}

impl From<::xmlrpc::Fault> for LoginError {
    fn from(err: ::xmlrpc::Fault) -> LoginError {
        LoginErrorKind::XmlRpcFault(err).into()
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
    pub sim_port: u16,
}

impl LoginResponse {
    fn extract_vector3(raw: &str) -> Result<Vector3<f32>, LoginError> {
        let re = Regex::new(r"\[r([0-9\.-]+),r([0-9\.-]+),r([0-9\.-]+)\]").unwrap();
        match re.captures(raw) {
            Some(caps) => {
                let x = try!(caps.get(1).unwrap().as_str().parse::<f32>());
                let y = try!(caps.get(2).unwrap().as_str().parse::<f32>());
                let z = try!(caps.get(3).unwrap().as_str().parse::<f32>());
                Ok(Vector3::new(x, y, z))
            }
            _ => Err(
                LoginErrorKind::ExtractResponseError(format!("vector3='{}'", raw)).into(),
            ),
        }
    }

    fn extract(response: BTreeMap<String, XmlValue>) -> Result<LoginResponse, LoginError> {
        fn err(msg: &'static str) -> LoginError {
            LoginErrorKind::ExtractResponseError(msg.to_string()).into()
        }

        // TODO: Check if additional items should be extracted.
        let look_at = match response.get("look_at") {
            Some(&XmlValue::String(ref raw)) => try!(LoginResponse::extract_vector3(raw)),
            _ => return Err(err("look_at")),
        };
        let circuit_code = match response.get("circuit_code") {
            Some(&XmlValue::Int(code)) => code as u32,
            _ => return Err(err("circuit_code")),
        };
        let session_id = match response.get("session_id") {
            Some(&XmlValue::String(ref id)) => try!(Uuid::parse_str(id)),
            _ => return Err(err("session_id")),
        };
        let agent_id = match response.get("agent_id") {
            Some(&XmlValue::String(ref id)) => try!(Uuid::parse_str(id)),
            _ => return Err(err("agent_id")),
        };
        let sim_ip = match response.get("sim_ip") {
            Some(&XmlValue::String(ref ip_raw)) => try!(Ip4Addr::from_str(ip_raw)),
            _ => return Err(err("sim_ip")),
        };
        let sim_port = match response.get("sim_port") {
            Some(&XmlValue::Int(port)) => port as u16,
            _ => return Err(err("sim_port")),
        };

        Ok(LoginResponse {
            look_at: look_at,
            circuit_code: circuit_code,
            session_id: session_id,
            agent_id: agent_id,
            sim_ip: sim_ip,
            sim_port: sim_port,
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
        data.insert(
            "passwd".to_string(),
            XmlValue::from(&self.password_hash[..]),
        );
        data.insert("start".to_string(), XmlValue::from(&self.start[..]));
        data.insert("version".to_string(), XmlValue::from("0.1.0"));
        data.insert("channel".to_string(), XmlValue::from("tokio-opensim"));
        data.insert("platform".to_string(), XmlValue::from("Linux"));

        let client = ::reqwest::Client::new()?;

        let value = ::xmlrpc::Request::new("login_to_simulator")
            .arg(XmlValue::Struct(data))
            .call(&client, url)??;

        match value {
            XmlValue::Struct(s) => LoginResponse::extract(s),
            _ => Err(
                LoginErrorKind::ExtractResponseError("value is not a struct".into()).into(),
            ),
        }
    }
}
