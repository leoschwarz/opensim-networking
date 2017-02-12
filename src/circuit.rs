use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode,
               WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE};

use std::net::{SocketAddr, SocketAddrV4};
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;

use tokio_core::reactor::Core;
use tokio_core::net::UdpSocket;
use futures::{Future, Stream, Sink};
use futures::sync::mpsc;

use time::Timespec;


/// We only consider viewer <-> simulator circuits.
/// NOTE: In the future we'll want to implement Ipv6 support once it lands in opensim.
pub struct Circuit {
    core: Core,

    /// The socket used for communication.
    socket: UdpSocket,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,

    /// Maybe A 24 bit number (TODO: change type here???) created at the connection of any circuit.
    /// This number is stored in the packet header and incremented whenever a packet is sent
    /// from one end of the circuit to the other.
    ///
    /// For each connection and each direction messages are numbered with unique numbers in a
    /// sequential fashion. This number is incremented after sending each message.
    sequence_number: u32,
}

#[derive(Debug)]
pub enum CircuitInitiationError {
    IO(::std::io::Error),
}

impl From<::std::io::Error> for CircuitInitiationError {
    fn from(err: ::std::io::Error) -> Self {
        CircuitInitiationError::IO(err)
    }
}

pub enum SendPacketError {
    /// The packet was to be sent reliable but not acknowledged in time.
    TimedOut,

    /// Any unresolved IoError instance.
    IoError(error: IoError),
}

pub enum SendPacketStatus {
    /// The packet was sent successfully.
    /// checked is true if and only if the package was sent with the reliable flag
    /// and acknowledged by the remote.
    Success(bool),

    /// There was an error sending the packet.
    Error(SendPacketError),

    /// Still waiting for a result.
    Pending
}

/// Return type for sending packets.
pub struct SendPacket {
}

impl SendPacket {
    pub fn status(&self) -> SendPacketStatus {
        // TODO: Implement.
        SendPacketStatus::Pending
    }
}

struct MessageManager {
    socket: UdpSocket,

    // TODO: Wrap Packets in a special struct which will keep track of the return status
    // and update it in an Arc or something like that so the SendPacket future will be able
    // to yield the correct value.
    queue_send: Sender<Packet>,
}

impl MessageManager {
    fn start(sim_address: SocketAddr) -> MessageManager {
        // Create a FIFO channel to send packets to the queue.
        // TODO: Determine a good buffer size or make it configurable.
        let (queue_send, queue_recv) = mpsc::channel(100);

        // Setup tokio.
        // 0.0.0.0:0 let's the OS chose an appropriate local UDP socket. TODO check.
        let mut core = Core::new().unwrap();
        let addr = SocketAddr::from_str("0.0.0.0:0");
        let socket = UdpSocket::bind(&addr, &core.handle());

        
        // TODO: Maybe we will actually have to `UdpFramed` here but I am not sure if
        // it is going to be an issue that UdpFramed::map will consume the instance.
        // Like will it be still possible in stream1 to write to the stream?

        
        // Stream 1:
        // 1. Read packets from the channel.
        // 2. Serialize the packets and send them through the socket.
        // 3. TODO: Register the packet if it is sent with the reliable flag.
        let stream1 = queue_recv.map(|packet| {
            let mut buf = Vec::new();
            packet.write_to(&mut buf);
            socket.send_dgram(buf, sim_address)
        }).map(|_| ());

        // Stream 2:
        // 1. Read datagrams from the socket.
        // 2. Decode the datagrams to packets.
        // 3. TODO: Register incoming acks.
        // 4. TODO: If the incoming packet has the reliable flag set, acknowledge it.
        // 5. Put the resulting packet into the output stream.
        let stream2 = socket.

        // Combine stream1 and stream2 using Stream::select.
        // As this is using a round-robin strategy this will make sure the stream allocation is
        // fair.



        // Create the sender thread.
        thread::spawn(move || {
            loop {



                // TODO: Remove unwrap().
                // Receive a packet to be sent from the queue.
                // If no packet is available this will block the thread until one becomes
                // available.
                let packet = queue_recv.recv().unwrap();

                // Serialize the packet.
                let mut buf = Vec::new();
                packet.write_to(&mut buf);

                // Send it through the socket.
                // TODO: Register reliable packages.
                // TODO: Handle return values.
                // TODO: Remove unwrap().
                socket.send_to(&buf, sim_address).unwrap();
            }
        });

        // Return the struct.
        MessageManager {
            queue_send: queue_send
        }
    }
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse)
                    -> Result<Circuit, CircuitInitiationError> {

        // Create the circuit instance.
        let circuit = Circuit {
            core: core,
            socket: socket,
            sim_address: sim_address,
            sequence_number: 1,
        };

        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone(),
            },
        };
        let mut packet = Packet::new(msg, 1);
        packet.enable_flags(PACKET_RELIABLE);
        circuit.send_packet(packet);

        // TODO: Wait for an ack.

        // Finished.
        Ok(circuit)
    }

    /// Send a packet to the simulator.
    fn send_packet(&self, packet: Packet) -> SendPacket {
        let mut buf = Vec::new();
        packet.write_to(&mut buf);

    }
}
