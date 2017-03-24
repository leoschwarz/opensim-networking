use login::LoginResponse;
use messages::MessageInstance;
use packet::{Packet, SequenceNumber, PACKET_RESENT};
use util::{AtomicU32Counter, mpsc_read_many};

use futures::{Async, Future, Poll};
use std::collections::HashMap;
use std::io::Error as IoError;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use time::{Duration, Timespec, get_time};

// Things not implemented for now.
// - Ipv6 support. (Blocked by opensim's support.)
// - Simulator <-> simulator circuits. (Do we need these?)

/// Encapsulates a so called circuit (networking link) between our viewer and a simulator.
pub struct Circuit {
    /// This instance actually manages the messages.
    message_manager: MessageManager,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse, config: CircuitConfig) -> Result<Circuit, IoError> {
        let sim_address = SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip, login_res.sim_port));
        Ok(Circuit {
               sim_address: sim_address,
               message_manager: MessageManager::start(sim_address, config)?,
           })
    }
}

pub struct CircuitConfig {
    /// The number of seconds before an unconfirmed packet becomes invalid.
    /// If multiple attempts are allowed, each single attempt will get at most this time before
    /// timing out.
    send_timeout: Duration,

    /// The number of times resending an unacknowledged packet before reporting it as failure.
    send_attempts: usize,
}

struct MessageManagerItem {
    packet: Packet,
    status: Arc<Mutex<SendMessageStatus>>,
}

#[derive(Debug, Clone, Copy)]
pub enum SendMessageError {
    /// Remote failed to acknowledge the packet.
    FailedAck,
}

#[derive(Debug, Clone, Copy)]
pub enum SendMessageStatus {
    /// Has not been sent through the socket yet.
    PendingSend,
    /// Has been sent but not acknowledged yet.
    /// The attempt variant describes the number of attempts already made.
    /// (0 → this is the first attempt, 1 → 2nd attempt, etc.)
    /// timeout: holds the time after which the current attemt is considered timed out.
    PendingAck { attempt: u8, timeout: Timespec },
    /// Has finished successfully.
    Success,
    /// Has failed.
    Failure(SendMessageError),
}

impl SendMessageStatus {
    fn next_status(&self, packet_reliable: bool, config: &CircuitConfig) -> SendMessageStatus {
        match *self {
            SendMessageStatus::PendingSend => {
                if packet_reliable {
                    SendMessageStatus::PendingAck {
                        attempt: 0,
                        timeout: get_time() + config.send_timeout,
                    }
                } else {
                    SendMessageStatus::Success
                }
            }
            SendMessageStatus::PendingAck { attempt, timeout } => {
                if attempt + 1 >= (config.send_attempts as u8) {
                    SendMessageStatus::Failure(SendMessageError::FailedAck)
                } else {
                    SendMessageStatus::PendingAck {
                        attempt: attempt + 1,
                        timeout: get_time() + config.send_timeout,
                    }
                }
            }
            _ => panic!("Invalid states."),
        }
    }

    pub fn is_failure(&self) -> bool {
        match *self {
            SendMessageStatus::Failure(_) => true,
            _ => false,
        }
    }

    fn is_resend(&self) -> bool {
        match *self {
            SendMessageStatus::PendingAck { attempt, .. } => attempt > 0,
            _ => false,
        }
    }
}

pub struct SendMessage {
    status: Arc<Mutex<SendMessageStatus>>,
}

impl Future for SendMessage {
    /// If we complete without error it means an unreliable packet was sent or a reliable
    /// packet was sent and ack'ed by the server.
    type Item = ();
    type Error = SendMessageError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.status.lock() {
            Ok(status) => {
                match *status {
                    SendMessageStatus::PendingSend => Ok(Async::NotReady),
                    SendMessageStatus::PendingAck { .. } => Ok(Async::NotReady),
                    SendMessageStatus::Success => Ok(Async::Ready(())),
                    SendMessageStatus::Failure(err) => Err(err),
                }
            }
            Err(_) => panic!("We must never panic if locking the status."),
        }
    }
}

/// Handles the sending and receiving of messages internally.
///
/// One thread is created to be perform requests concurrently to the rest of the program.
///
/// TODO: Figure out a good API to notify on both of these events:
/// - received a new package from the circuit.
/// - a package has timed out too many times and cannot be retried anymore.
///
/// TODO: If we want to allow for thread safe concurrent access from multiple threads we
///  will have to create something like a writer struct that can be sent over to other threads.
///  The problem with API design here is that we will have to export some kind of structs,
///  e.g. `MessageReader` and `MessageWriter` and pass these through `Circuit`'s API with methods
///  like `message_reader()` and `message_writer()`.
///  It's certainly a possibility but there might be a better way.
///
/// TODO: How will it affect us if the other side ignores our acks, will we be fine just resending
///  the ack for a resent package.
///
/// TODO: Add a way to send messages without retrying them.
///  This could become useful when for example a navigation packet gets lost and a user is already
///  moving into a different location and would be sent back if this old packet would be resent
///  now.
struct MessageManager {
    incoming: mpsc::Receiver<MessageInstance>,
    outgoing: mpsc::Sender<MessageManagerItem>,
    sequence_counter: AtomicU32Counter,
}

impl MessageManager {
    fn send_message<M: Into<MessageInstance>>(&self, msg: M, reliable: bool) -> SendMessage {
        let mut packet = Packet::new(msg.into(), self.sequence_counter.next());
        packet.set_reliable(reliable);

        let status = Arc::new(Mutex::new(SendMessageStatus::PendingSend));
        let item = MessageManagerItem {
            packet: packet,
            status: status.clone(),
        };
        self.outgoing.send(item).unwrap();

        SendMessage { status: status }
    }

    fn start(sim_address: SocketAddr, config: CircuitConfig) -> Result<MessageManager, IoError> {
        // Setup communication channels.
        // TODO: Consider a better data structure for the incoming acks. (write_many support)
        let (incoming_tx, incoming_rx) = mpsc::channel::<MessageInstance>();
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<MessageManagerItem>();
        let (acks_outgoing_tx, acks_outgoing_rx) = mpsc::channel::<SequenceNumber>();
        let (acks_incoming_tx, acks_incoming_rx) = mpsc::channel::<SequenceNumber>();
        let (register_packet_tx, register_packet_rx) = mpsc::channel::<MessageManagerItem>();

        // TODO: As we will be using blocking IO in the reader and writer thread
        //       figure out a safe way to stop the threads within "constant time".

        // Create sockets.
        let socket_out = UdpSocket::bind("0.0.0.0:0")?;
        socket_out.set_read_timeout(None);
        socket_out.set_nonblocking(false);
        let socket_in = socket_out.try_clone()?;

        // Create sender thread.
        thread::spawn(move || {

            loop {
                // Blocking read of the next outgoing item.
                match outgoing_rx.recv() {
                    Ok(item) => {
                        let (mut packet, mut status) = (item.packet, item.status);
                        // Append pending acks.
                        if packet.appended_acks.is_empty() {
                            packet.appended_acks = mpsc_read_many(&acks_outgoing_rx, 255);
                        }

                        // Determine the next status.
                        let current_status = status.lock().unwrap();
                        let next_status = current_status.next_status(packet.is_reliable(), &config);

                        // Send if there isn't a failure (i.e. too many attempts resending the
                        // packet).
                        if !next_status.is_failure() {
                            // Mark the packet as resend if it is a resend.
                            if next_status.is_resend() {
                                packet.enable_flags(PACKET_RESENT);
                            }

                            let mut buf = Vec::<u8>::new();
                            packet.write_to(&mut buf);
                            socket_out.send_to(&buf, sim_address);
                        }

                        // Update item's status.
                        // TODO: This doesn't work yet.
                        let status_mut = status.get_mut().unwrap();
                        *status_mut = next_status;

                        // TODO: Register the item for timeout checking.
                    }
                    Err(_) => panic!("channel closed"),
                }
            }
        });

        // Create reader thread.
        thread::spawn(move || {
            let mut buf = Vec::<u8>::new();

            loop {
                // Read from socket in blocking way.
                buf.clear();
                socket_in.recv(&mut buf).unwrap();

                // Parse the packet.
                let mut packet = Packet::read(&buf).unwrap();

                // Read appended acks and send ack if requested (reliable packet).
                for ack in &packet.appended_acks {
                    acks_incoming_tx.send(*ack).unwrap();
                }
                if packet.is_reliable() {
                    acks_outgoing_tx.send(packet.sequence_number).unwrap();
                }

                // Yield the received message.
                incoming_tx.send(packet.message).unwrap();
            }
        });

        // Create ack book-keeping thread.
        thread::spawn(move || loop {
                          let item = register_packet_rx.recv().unwrap();

                      });


        Ok(MessageManager {
               incoming: incoming_rx,
               outgoing: outgoing_tx,
               sequence_counter: AtomicU32Counter::new(0),
           })
    }
}
