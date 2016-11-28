use {Ip4Addr, Ip4Port, Uuid};

use messages::{UseCircuitCode, UseCircuitCode_CircuitCode};
use login::LoginResponse;


// TODO: Generate these implementations automatically later.
use messages::Message;
use std::io::Write;
use byteorder::{LittleEndian, WriteBytesExt};
use messages::WriteMessageResult;
impl Message for UseCircuitCode {
    fn write_to<W: Write>(&self, buffer: &mut W) -> WriteMessageResult {
        try!(buffer.write_u32::<LittleEndian>(self.circuit_code.code));
        try!(buffer.write(self.circuit_code.session_id.as_bytes()));
        try!(buffer.write(self.circuit_code.id.as_bytes()));
        Ok(())
    }
}

/// We only consider viewer <-> simulator circuits.
pub struct Circuit {
    ip: Ip4Addr,
    port: Ip4Port,
}

//fn send_packet(

impl Circuit {
    fn initiate(login_res: &LoginResponse) {
        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone()
            }
        };

        // Send the packet and wait for ack.



    }
}

