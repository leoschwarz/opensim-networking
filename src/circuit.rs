use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode,
               WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE};

use std::net::{SocketAddr, SocketAddrV4};
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::str::FromStr;

use tokio_core::reactor::Core;
use tokio_core::net::{UdpCodec, UdpSocket};
use futures::{Future, Poll, Stream, Sink};
use futures::stream::BoxStream;
use futures::sync::mpsc;

use time::Timespec;


/// We only consider viewer <-> simulator circuits.
/// NOTE: Implement Ipv6 support once it lands in opensim.
pub struct Circuit {
    /// This instance actually manages the messages.
    message_manager: MessageManager,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,

}

impl Circuit {
    pub fn initiate(login_res: LoginResponse)
                    -> Result<Circuit, CircuitInitiationError> {

        let sim_address = SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip,
                                                           login_res.sim_port));

        // Create the message manager instance.
        let mut message_manager = MessageManager::start(sim_address, 100);

        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone(),
            },
        };
        message_manager.send_message(msg, true);

        // TODO: Wait for an ack.

        // Create the circuit instance.
        let circuit = Circuit {
            sim_address: sim_address,
            message_manager: message_manager
        };

        // Finished.
        Ok(circuit)
    }
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
    IoError(IoError),
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

#[derive(Debug)]
enum MessageManagerItemError {
    FailedSendingPacket,
    IncomingFailed
}

/// Provides message management.
/// This includes correct sending of messages and retrying them if they are sent with the reliable
/// flag, but not acknowledged in time.
/// This also includes receiving of messages and acknowledging messages to the simulator.
struct MessageManager {
    // TODO: Wrap Packets in a special struct which will keep track of the return status
    // and update it in an Arc or something like that so the SendPacket future will be able
    // to yield the correct value.
    /// Outgoing packets to be sent yet.
    queue_outgoing: mpsc::Sender<Packet>,
    /// Incoming packets to be read yet.
    queue_incoming: mpsc::Receiver<Packet>,

    /// TODO: Figure out if this is really a u32 or a u24 like it was stated in some docs.
    ///
    /// Sequence nmubers keep track of the packets sent and are unique to each packet and each
    /// direction, they are incremented by one after a package send. This field holds the sequence
    /// number of the package that was just sent. (0=none was sent before.)
    sequence_number: u32,
}

impl MessageManager {
    fn start(sim_address: SocketAddr,
             buffer_size: usize) -> MessageManager {
        use futures::stream::*;

        // Create channels for outgoing and incoming packets.
        let (outgoing_send, outgoing_recv) = mpsc::channel(buffer_size);
        let (mut incoming_send, incoming_recv) = mpsc::channel(buffer_size);

        // Create the sender thread.
        thread::spawn(move || {
            // Create the event loop core.
            let mut core = Core::new().unwrap();

            // Create the socket.
            // TODO: Check that 0.0.0.0:0 actually lets the OS chose an appropriate local UDP socket.
            let addr = SocketAddr::from_str("0.0.0.0:0").unwrap();
            let socket_raw = UdpSocket::bind(&addr, &core.handle()).unwrap();

            // Frame the socket so that we can directly supply and read Packets to and from it.
            let socket = socket_raw.framed(OpensimCodec::new(sim_address));
            let (mut socket_sink, socket_stream) = socket.split();

            // Stream 1:
            // 1. Read packets from the outgoing queue.
            // 2. Send the packet through the (framed) socket.
            // 3. TODO: Register the packet if it is sent with the reliable flag.
            let stream1 = outgoing_recv
                .map(move |packet| socket_sink.start_send(packet))
                .map(|_| ())
                .map_err(|_| MessageManagerItemError::FailedSendingPacket).boxed();
                //.map_err(|_| IoError::new(IoErrorKind::Other, "Unknown error in stream1.")); // TODO: Don't swallow errors.

            // Stream 2:
            // 1. Read packets from the (framed) socket.
            // 3. TODO: Register incoming acks.
            // 4. TODO: If the incoming packet has the reliable flag set, acknowledge it.
            // 5. Put the resulting packet into the incoming queue.
            let stream2 = socket_stream
                .map(move |packet| incoming_send.start_send(packet)) // TODO: make sure we call .poll_complete()
                .map(|_| ())
                .map_err(|_| MessageManagerItemError::IncomingFailed).boxed();
                //.map_err(|_| ()); // TODO: Don't swallow errors.

            // Combine stream1 and stream2 using Stream::select.
            // As this is using a round-robin strategy scheduling between stream1 and stream2 will be
            // fair.
            let combined: BoxStream<(), MessageManagerItemError> = stream1.select(stream2).boxed();

            // Run the main event loop.
            match core.run(combined.into_future()) {
                Ok(_) => {},
                Err(_) => { panic!("There was an error."); }
            };
        });

        // Return the struct.
        MessageManager {
            queue_outgoing: outgoing_send,
            queue_incoming: incoming_recv,
            sequence_number: 0
        }
    }

    fn send_message<M: Into<MessageInstance>>(&mut self, message: M, reliable: bool)
        -> Poll<(), mpsc::SendError<Packet>>
    {
        // Create the packet.
        let mut packet = Packet::new(message, self.next_sequence_number());
        packet.set_reliable(reliable);

        // Put it into the send queue.
        self.queue_outgoing.start_send(packet);
        self.queue_outgoing.poll_complete()
    }

    /* TODO implement the efficient (call poll_complete() only once for a whole vec of messages)
     *  version later. Returning a proper future which can also handle acks is more important.
    fn send_messages(&mut self, messages: Vec<MessageInstance>, reliable: bool)
        -> Poll<(), mpsc::SendError<Packet>>
    {
        for message in messages {
            // Create the packet.
            let mut packet = Packet::new(message, self.next_sequence_number());
            packet.set_reliable(reliable);

            // Put it into the send queue.
            self.queue_outgoing.start_send(packet);
        }

        // Make sure that all items are sent to the queue.
        self.queue_outgoing.poll_complete()
    }
    */

    #[inline]
    fn next_sequence_number(&mut self) -> u32 {
        self.sequence_number += 1;
        self.sequence_number
    }
}

struct OpensimCodec {
    sim_address: SocketAddr
}

impl OpensimCodec {
    fn new(sim: SocketAddr) -> OpensimCodec {
        OpensimCodec {
            sim_address: sim
        }
    }
}

impl UdpCodec for OpensimCodec {
    type In = Packet;
    type Out = Packet;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> Result<Self::In, ::std::io::Error> {
        Packet::read(buf)
    }

    fn encode(&mut self, packet: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {
        packet.write_to(buf);
        self.sim_address
    }
}

