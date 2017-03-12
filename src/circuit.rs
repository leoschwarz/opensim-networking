use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode,
               WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE, SequenceNumber};
use util::mpsc_read_many;

use std::net::{SocketAddr, SocketAddrV4};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::str::FromStr;

use tokio_core::reactor::Core;
use tokio_core::net::{UdpCodec, UdpSocket};
use tokio_timer;
use tokio_timer::Timer;
use futures::{Future, Poll, Stream, Sink, Async};
use futures::stream::BoxStream;
use futures::sync::mpsc;

//use time::{Duration, Timespec};
use std::time::Duration;
//use ttl_cache::TtlCache;

// Things not implemented for now.
// - Ipv6 support. (Blocked by opensim's support.)
// - Simulator <-> simulator circuits. (Do we need these?)

/// Encapsulates a so called circuit (networking link) between our viewer
/// and a simulator.
pub struct Circuit {
    /// This instance actually manages the messages.
    message_manager: MessageManager,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse) -> Result<Circuit, CircuitInitiationError> {

        let sim_address = SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip, login_res.sim_port));

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
            message_manager: message_manager,
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

#[derive(Debug)]
enum SendMessageStatus {
    /// Has not been sent yet.
    Pending,
    /// Still waiting for an ack.
    WaitingAck,
    /// Everything was successful.
    /// If an ack was requested it was also received.
    Success,
    /// There was a failure sending the packet.
    Failure(SendMessageError),
}

#[derive(Debug)]
pub enum SendMessageError {
    /// For some reason the status flag of the `SendMessage` struct could not have been read.
    CantReadStatus,

    /// Even after the maximum number of attempts the package was not acknowledged.
    FailedAck,

    /// TODO: Maybe get rid of this. Because it could come from either the socket or be a
    /// serialization error.
    IoError(IoError),
}

/// Future return type for sending packets.
pub struct SendMessage<'a> {
    packet: Packet,
    status: RwLock<SendMessageStatus>,
    timer: &'a Timer,
}

impl<'a> SendMessage<'a> {
    fn new(packet: Packet, timer: &Timer) -> SendMessage {
        SendMessage {
            packet: packet,
            status: RwLock::new(SendMessageStatus::Pending),
            timer: timer,
        }
    }
}

impl<'a> Future for SendMessage<'a> {
    type Item = ();
    type Error = SendMessageError;

    // TODO: Figure out if it's an issue that "work" (here this only means to continue
    // waiting for an ack).
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.status.read() {
            Ok(status) => {
                match *status {
                    SendMessageStatus::WaitingAck => Ok(Async::NotReady),
                    SendMessageStatus::Pending => Ok(Async::NotReady),
                    SendMessageStatus::Success => Ok(Async::Ready(())),
                    SendMessageStatus::Failure(e) => Err(e),
                }
            }
            // This should only be the case if the lock was poisoned.
            Err(_) => Err(SendMessageError::CantReadStatus),
        }
    }
}

#[derive(Debug)]
enum MessageManagerItemError {
    FailedSendingPacket,
    IncomingFailed,
}

struct MessageManagerConfig {
    /// The number of seconds before an unconfirmed packet becomes invalid.
    /// If multiple attempts are allowed, each single attempt will get at most this time before
    /// timing out.
    send_timeout: Duration,

    /// The number of times resending an unacknowledged packet before reporting it as failure.
    send_attempts: usize,
}

impl Default for MessageManagerConfig {
    fn default() -> Self {
        MessageManagerConfig {
            send_timeout: Duration::from_secs(5),
            send_attempts: 3,
        }
    }
}

/// Provides message management.
/// This includes correct sending of messages and retrying them if they are sent with the reliable
/// flag, but not acknowledged in time.
/// This also includes receiving of messages and acknowledging messages to the simulator.
struct MessageManager {
    /// Outgoing packets to be sent yet.
    queue_outgoing: mpsc::Sender<Packet>,
    /// Incoming packets to be read yet.
    queue_incoming: mpsc::Receiver<Packet>,

    /// TODO: Figure out if this is really a u32 or a u24 like it was stated in some docs.
    ///
    /// Sequence numbers keep track of the packets sent and are unique to each packet and each
    /// direction, they are incremented by one after a package send. This field holds the sequence
    /// number of the package that was just sent. (0=none was sent before.)
    sequence_number: SequenceNumber,

    config: MessageManagerConfig,

    /// The timer used to check reliable packets' acks and to resend them if needed.
    timer: Timer,
}

impl MessageManager {
    fn start(sim_address: SocketAddr, buffer_size: usize) -> MessageManager {
        use futures::stream::*;

        let config = MessageManagerConfig::default();

        // TODO: Rewrite this function. Consider if would make it less confusing
        // to store each channel in only a single tuple and directly access the
        // values from these tuples. (tuple.0 and tuple.1)

        // Create channels for outgoing and incoming packets.
        let (outgoing_send, outgoing_recv) = mpsc::channel(buffer_size);
        let (mut incoming_send, incoming_recv) = mpsc::channel(buffer_size);

        // Create channels for incoming and outgoing acks.
        let (acks_out_send, acks_out_recv) = mpsc::channel(buffer_size);
        let (mut acks_in_send, acks_in_recv) = mpsc::channel(buffer_size);

        // Create the timer instance.
        // TODO: Check all possible configuration options and optimize.
        let timer = tokio_timer::wheel().max_capacity(65536).build();

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

            // Setup the ack manager.
            //let mut ack_manager = AckManager::new();

            // Stream 1:
            // 1. Read packets from the outgoing queue.
            // 2. Send the packet through the (framed) socket.
            // 3. TODO: Register the packet if it is sent with the reliable flag.
            let stream1 = outgoing_recv
                .map(move |packet| {
                    socket_sink.start_send(packet);
                    socket_sink.poll_complete() // TODO: Maybe we can fold all of the stream into just this poll future?
                })
                .map(|_| ())
                .map_err(|_| MessageManagerItemError::FailedSendingPacket).boxed();
            //.map_err(|_| IoError::new(IoErrorKind::Other, "Unknown error in stream1.")); // TODO: Don't swallow errors.

            // Stream 2:
            // 1. Read packets from the (framed) socket.
            // 3. TODO: Register incoming acks.
            // 4. TODO: If the incoming packet has the reliable flag set, acknowledge it.
            // 5. Put the resulting packet into the incoming queue.
            let stream2 = socket_stream.map(move |packet: Packet| {
                    // Register incoming acks.
                    if !packet.appended_acks.is_empty() {
                        // TODO acks_in_send.
                    }

                    // Register the incoming packet.
                    incoming_send.start_send(packet);
                    incoming_send.poll_complete() // TODO
                })
                .map(|_| ())
                .map_err(|_| MessageManagerItemError::IncomingFailed)
                .boxed();
            //.map_err(|_| ()); // TODO: Don't swallow errors.

            // Combine stream1 and stream2 using Stream::select.
            // As this is using a round-robin strategy scheduling between stream1 and stream2 will be
            // fair.
            let combined: BoxStream<(), MessageManagerItemError> = stream1.select(stream2).boxed();

            // Run the main event loop.
            match core.run(combined.into_future()) {
                Ok(_) => {}
                Err(_) => {
                    panic!("There was an error.");
                }
            };
        });

        // Return the struct.
        MessageManager {
            queue_outgoing: outgoing_send,
            queue_incoming: incoming_recv,
            sequence_number: 0,
            timer: timer,
            config: config,
        }
    }

    fn send_message<M: Into<MessageInstance>>(&mut self,
                                              message: M,
                                              reliable: bool)
                                              -> SendMessage {
        // Create the packet.
        let mut packet = Packet::new(message, self.next_sequence_number());
        packet.set_reliable(reliable);

        // Create the future.
        let send = SendMessage::new(packet, &self.timer);
        send


        // TODO: Attempt multiple times if sending reliably.
        // Determine if this belongs into the SendMessage::poll() or do I do this here instead?

        // Put it into the send queue.
        //self.queue_outgoing.start_send(packet);
        //self.queue_outgoing.poll_complete()
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
    fn next_sequence_number(&mut self) -> SequenceNumber {
        self.sequence_number += 1;
        self.sequence_number
    }
}

struct OpensimCodec {
    sim_address: SocketAddr,
}

impl OpensimCodec {
    fn new(sim: SocketAddr) -> OpensimCodec {
        OpensimCodec { sim_address: sim }
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
