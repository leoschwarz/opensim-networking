use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode,
               WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE, SequenceNumber};

use std::net::{SocketAddr, SocketAddrV4};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::str::FromStr;

use tokio_core::reactor::Core;
use tokio_core::net::{UdpCodec, UdpSocket};
use futures::{Future, Poll, Stream, Sink, Async};
use futures::stream::BoxStream;
use futures::sync::mpsc;

use time::{Duration, Timespec};
use ttl_cache::TtlCache;


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
    Failure(SendMessageError)
}

pub enum SendMessageError {
    /// For some reason the status flag of the `SendMessage` struct could not have been read.
    CantReadStatus,

    /// Even after the maximum number of attempts the package was not acknowledged.
    FailedAck,

    /// TODO: Maybe get rid of this. Because it could come from either the socket or be a
    /// serialization error.
    IoError(IoError),
}

/// Return-type for sending packets.
pub struct SendMessage {
    packet: Packet,
    status: RwLock<SendMessageStatus>
}

impl SendMessage {
    fn new(packet: Packet) -> SendMessage {
        SendMessage {
            packet: packet,
            status: RwLock::new(SendMessageStatus::Pending)
        }
    }
}

impl Future for SendMessage {
    Item = ();
    Error = SendMessageError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.status.read() {
            Ok(SendMessageStatus::WaitingAck) => Ok(Async::NotReady),
            Ok(SendMessageStatus::Pending) => Ok(Async::NotReady),
            Ok(SendMessageStatus::Success) => Ok(Async::Ready(())),
            Ok(SendMessageStatus::Failure(e)) => Err(e),
            Err(_) => Err(SendMessageError::CantReadStatus)
        }
    }
}

#[derive(Debug)]
enum MessageManagerItemError {
    FailedSendingPacket,
    IncomingFailed
}

/// The AckWaitlist is based on a circular buffer in an array, which keeps items sorted by insert
/// time. As the sequence number grows monotonically chances are there will be an overflow, but if
/// there is any we'll be able to treat it by keeping a special pointer to the first value that is
/// starting at a low value again.
///
/// TODO: This is also the reason why we aren't using a pre-made ring buffer.
///
/// It is possible to periodically check the list's first item and see if the packet is outdated.
/// If it is outdated it will be put onto the retry list. The idea being that in general such
/// packages will not have to be around at all times.
struct AckWaitlist {
    capacity: usize,

    // TODO: How to handle empty and fùll buffer. (Do we need a special flag for either of the
    // cases?)

    // TODO: Packete zu speichern ist wahrscheinlich ein Problem, weil man in Folge mehr speicher
    // braucht und der ganze Puffer nicht mehr in den Cache passen wird. Man könnte sich überlegen,
    // ob es nicht mehr Sinn machen würde, das ganze in den richtigen Pointer zu legen.
    //
    // Es muss an mehreren Stellen verwendet werden können, das würde auch vermeiden, dass man für
    // `SendMessage` das gesamte Packet kopieren muss. (Man würde dann nur den Pointer kopieren
    // müssen.)
    //
    // Was ich noch nicht weiss:
    // - Welcher Pointer: Kandidaten sind sicher Rc oder Arc, aber vielleicht gibt es noch etwas
    // besseres das man verwenden könnte?
    // - Wird es je nötig sein das Packet zu mutieren, wenn ja wird man das ganze nämlich noch in
    // eine `RefCell` packen müssen.
    regular_items: Vec<Packet>
    regular_first: usize,
    regular_last: usize,
    /// If Some(i) this points to the first value of an overflow. i.e. the first value, which
    /// against the invariant of the items being sorted in ascending order is smaller than its
    /// predecessor.
    ///
    /// This struct should only ever be used in such a way that there is at most one such
    /// occurence.
    overflow_item: Option<usize>

    /// An unsorted list of packages which have been resent and are now waiting for a confirmation.
    resent_items: Vec<Packet>
}

impl AckWaitlist {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        AckWaitlist {
            capacity: capacity,
            regular_items: Vec::with_capacity(capacity),
            regular_first: 0,
            regular_last: 0,
            overflow_item: None,
            resent_items: Vec::new()
        }
    }

    fn len(&self) -> {
        
    }
}

struct AckManagerItem {
    packet: Packet,
    inserted_at: Timespec
}

struct AckManager {
    /// Registered packets still waiting for an ack.
    items: HashMap<SequenceNumber, AckManagerItem>,
    /// Incoming acks that have been registered from incoming packages.
    /// We are using a `TtlCache` here since the acks will be valid only for a limited time span.
    incoming_acks: TtlCache<SequenceNumber, ()>
}

impl AckManager {
    fn new() -> AckManager {
        // TODO: Make configurable.
        // The number of seconds before an ack becomes invalid.
        let ack_max_age = Duration::seconds(20);
        let ack_cache_size = 16000;

        AckManager {
            packets: BTreeMap::new(),
            incoming_acks: TtlCache::new(ack_max_age, ack_cache_size)
        }
    }

    /// Remove timed out packets from the manager.
    fn remove_timedout(&mut self) {
        // Determine the timed out packets' sequence numbers.
        let packet_nums: Vec<SequenceNumber> = self.items
            .iter()
            .filter(|&(_, item)| item.wait_until < ::time::get_time())
            .map(|(k, _)| k)
            .collect();

        // TODO: Depending on the design of the `SendMessage` future we will have to inform the
        // future manually that the packet has timed out. (But depending on the implementation this
        // is also something that could be done using polling.)

        // Now remove the items.
        for packet_num in packet_nums {
            self.items.remove(&packet_num);
        }
    }

    /// Register an ack as received.
    /// Acks for packets which were not registered before registering the ack will be ignored
    /// quietly.
    fn register_ack(&mut self, ack: SequenceNumber) {
        match self.items.remove(ack) {
            Some(item) => {
                // TODO: Notify the correct future, that the package was acknowledged.
                // TODO: Is it important to check if the package dated out here or should we
                // consider ourselves lucky and just notify the future if a package has been
                // acknowledged in time anyway.
            },
            None => {}
        }
    }

    /// Register a packet waiting for an ack.
    fn register_packet(&mut self, packet: Packet, wait: Duration) {
        self.items.insert(
            packet.sequence_number,
            AckManagerItem {
                packet: packet,
                wait_until: ::time::get_time() + wait
            }
        );
    }
}

/// Provides message management.
/// This includes correct sending of messages and retrying them if they are sent with the reliable
/// flag, but not acknowledged in time.
/// This also includes receiving of messages and acknowledging messages to the simulator.
struct MessageManager {
    // TODO: Wrap Packets in a special struct which will keep track of the return status
    // and update it in an Arc or something like that so the SendMessage future will be able
    // to yield the correct value.
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
}

impl MessageManager {
    fn start(sim_address: SocketAddr,
             buffer_size: usize) -> MessageManager {
        use futures::stream::*;

        // Create channels for outgoing and incoming packets.
        let (outgoing_send, outgoing_recv) = mpsc::channel(buffer_size);
        let (mut incoming_send, incoming_recv) = mpsc::channel(buffer_size);

        // Create channels for incoming and outgoing acks.
        let (acks_out_send, acks_out_recv) = mpsc::channel(buffer_size);
        let (mut acks_in_send, acks_in_recv) = mpsc::channel(buffer_size);

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
            let mut ack_manager = AckManager::new();

            // Stream 1:
            // 1. Read packets from the outgoing queue.
            // 2. Send the packet through the (framed) socket.
            // 3. TODO: Register the packet if it is sent with the reliable flag.
            let stream1 = outgoing_recv
                .map(move |packet| {
                    socket_sink.start_send(packet);
                    socket_sink.poll_complete()) // TODO: Maybe we can fold all of the stream into just this poll future?
                })
                .map(|_| ())
                .map_err(|_| MessageManagerItemError::FailedSendingPacket).boxed();
                //.map_err(|_| IoError::new(IoErrorKind::Other, "Unknown error in stream1.")); // TODO: Don't swallow errors.

            // Stream 2:
            // 1. Read packets from the (framed) socket.
            // 3. TODO: Register incoming acks.
            // 4. TODO: If the incoming packet has the reliable flag set, acknowledge it.
            // 5. Put the resulting packet into the incoming queue.
            let stream2 = socket_stream
                .map(move |packet: Packet| {
                    // Register incoming acks.
                    if !packet.appended_acks.is_empty() {
                        acks_in_send.
                    }

                    // Register the incoming packet.
                    incoming_send.start_send(packet);
                    incoming_send.poll_complete() // TODO
                })
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

    fn send_message<M: Into<MessageInstance>> (&mut self, message: M, reliable: bool) -> SendMessage
    {
        // Create the packet.
        let mut packet = Packet::new(message, self.next_sequence_number());
        packet.set_reliable(reliable);

        // Create the future.
        let send = SendMessage::new(packet);
        
        send


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
    fn next_sequence_number(&mut self) -> SequenceNumber {
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

