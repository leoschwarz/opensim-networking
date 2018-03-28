use crypto::digest::Digest;
use crypto::md5::Md5;
use failure::Error;
use regex::Regex;
use std::collections::BTreeMap;
use std::str::FromStr;
use types::{Ip4Addr, Uuid, Vector3};
use url::Url;
use xmlrpc::Value as XmlValue;

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

#[derive(Debug, Fail)]
pub enum LoginError {
    #[fail(display = "There was a network error: {}", 0)]
    Network(Error),

    #[fail(display = "Parsing the response failed: {}", 0)]
    ParseResponse(Error),

    #[fail(display = "Login failed due to server denying it: {:?}", 0)]
    LoginDenied(#[cause] ::xmlrpc::Fault),
}

#[derive(Clone, Debug)]
pub struct LoginResponse {
    pub look_at: Vector3<f32>,
    pub circuit_code: u32,
    pub session_id: Uuid,
    pub agent_id: Uuid,

    /// The URL where capabilities can be queried.
    pub seed_capability: Url,

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
                let x = f32::from_str(&caps[1]).map_err(|e| LoginError::ParseResponse(e.into()))?;
                let y = f32::from_str(&caps[2]).map_err(|e| LoginError::ParseResponse(e.into()))?;
                let z = f32::from_str(&caps[3]).map_err(|e| LoginError::ParseResponse(e.into()))?;
                Ok(Vector3::new(x, y, z))
            }
            _ => Err(LoginError::ParseResponse(format_err!(
                "Invalid vector3: '{}'",
                raw
            ))),
        }
    }

    fn extract(response: BTreeMap<String, XmlValue>) -> Result<LoginResponse, LoginError> {
        fn err(msg: &'static str) -> LoginError {
            LoginError::ParseResponse(format_err!("Missing response field: {}", msg))
        }

        // TODO: Check if additional items should be extracted.
        let look_at = match response.get("look_at") {
            Some(&XmlValue::String(ref raw)) => LoginResponse::extract_vector3(raw)?,
            _ => return Err(err("look_at")),
        };
        let circuit_code = match response.get("circuit_code") {
            Some(&XmlValue::Int(code)) => code as u32,
            _ => return Err(err("circuit_code")),
        };
        let session_id = match response.get("session_id") {
            Some(&XmlValue::String(ref id)) => {
                Uuid::parse_str(id).map_err(|e| LoginError::ParseResponse(e.into()))?
            }
            _ => return Err(err("session_id")),
        };
        let agent_id = match response.get("agent_id") {
            Some(&XmlValue::String(ref id)) => {
                Uuid::parse_str(id).map_err(|e| LoginError::ParseResponse(e.into()))?
            }
            _ => return Err(err("agent_id")),
        };
        let seed_capability = match response.get("seed_capability") {
            Some(&XmlValue::String(ref u)) => {
                Url::parse(u).map_err(|e| LoginError::ParseResponse(e.into()))?
            }
            _ => return Err(err("seed_caps")),
        };
        let sim_ip = match response.get("sim_ip") {
            Some(&XmlValue::String(ref ip_raw)) => {
                Ip4Addr::from_str(ip_raw).map_err(|e| LoginError::ParseResponse(e.into()))?
            }
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
            seed_capability: seed_capability,
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

        let client = ::reqwest::Client::new();

        let value = ::xmlrpc::Request::new("login_to_simulator")
            .arg(XmlValue::Struct(data))
            .call(&client, url)
            .map_err(|e| LoginError::Network(e.into()))?
            .map_err(|e| LoginError::LoginDenied(e))?;

        match value {
            XmlValue::Struct(s) => LoginResponse::extract(s),
            _ => Err(LoginError::ParseResponse(format_err!(
                "value is not a struct"
            ))),
        }
    }
}
