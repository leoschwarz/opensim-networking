//! Circuit and message management for viewer <-> server communication.
//!
//! # Message handlers
//!
//! When a message is received, it will be attempted to find a handler for this message.
//!
//! First it will be checked if there is a handler specific to the `MessageType` of the message,
//! if none is found, handlers with a filter will be queried for the message (TODO implement when needed),
//! giving the message to the first handler found to accept it.
//!
//! If no handler was found for a message, it will remain in the queue and can be received from
//! the Circuit with the `read` and `try_read` functions.
//!
//! # Backlog (TODO)
//!
//! - IPv6 support (blocked by OpenSim support)

// TODO:
// - Proper shutdown of circuit
// → This should be accompanied by a systems module providing functionality
// to send the correct messages to the sim to make sure the agent is
// actually disconnected from the sim and doesn't end up failing the next
// authentication. - Figure out max packet size and apply the value to our read
// and write buffers. - Make sure acks are not sent twice?
// - Do acks need to be sent with a reliable packet?
// - Make sure the code is free from deadlock and starvation.
// - Improve error handling.
// - Once the rest is done: cleanup + verify corectness.
// - Eliminate all unwraps from this module except where we can verify it will
// never fail.

use logging::Log;
use messages::MessageInstance;
use packet::Packet;
use simulator::ConnectInfo;
use types::SequenceNumber;
use util::FifoCache;

use std::error::Error;
use std::io::Error as IoError;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use std::fmt;

mod ack_manager;
use self::ack_manager::AckManagerTx;

mod status;
pub use self::status::{SendMessage, SendMessageError};
use self::status::SendMessageStatus;

// TODO: Rename to message_handlers or msg_handlers ?
pub mod handlers;
pub use self::handlers::MessageHandlers;
use self::handlers::MessageSender;

#[derive(Debug)]
pub enum ReadMessageError {
    Disconnected,
    Timeout,
}

impl fmt::Display for ReadMessageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for ReadMessageError {
    fn description(&self) -> &str {
        match *self {
            ReadMessageError::Disconnected => "MPSC reader was disconnected.",
            ReadMessageError::Timeout => "MPSC reader timed out.",
        }
    }
}

impl From<mpsc::RecvError> for ReadMessageError {
    fn from(_: mpsc::RecvError) -> Self {
        ReadMessageError::Disconnected
    }
}

impl From<mpsc::RecvTimeoutError> for ReadMessageError {
    fn from(e: mpsc::RecvTimeoutError) -> Self {
        match e {
            mpsc::RecvTimeoutError::Disconnected => ReadMessageError::Disconnected,
            mpsc::RecvTimeoutError::Timeout => ReadMessageError::Timeout,
        }
    }
}

impl From<mpsc::TryRecvError> for ReadMessageError {
    fn from(_: mpsc::TryRecvError) -> Self {
        ReadMessageError::Disconnected
    }
}

/// Encapsulates a so called circuit (networking link) between our viewer and a
/// simulator.
///
/// TODO:
/// - Stop/exit functionality.
pub struct Circuit {
    incoming: mpsc::Receiver<MessageInstance>,
    ackmgr_tx: AckManagerTx,
}

impl Circuit {
    pub fn initiate(
        connect_info: &ConnectInfo,
        config: CircuitConfig,
        message_handlers: MessageHandlers,
        log: Log,
    ) -> Result<Circuit, IoError> {
        let sim_address = SocketAddr::V4(SocketAddrV4::new(
            connect_info.sim_ip,
            connect_info.sim_port,
        ));

        // Queue for incoming messages.
        let (incoming_tx, incoming_rx) = mpsc::channel::<MessageInstance>();

        // Create sockets.
        let socket_out = UdpSocket::bind("0.0.0.0:0")?;
        socket_out.connect(sim_address)?;
        socket_out.set_read_timeout(None)?;
        socket_out.set_nonblocking(false)?;
        let socket_in = socket_out.try_clone()?;

        // Setup AckManager.
        let (ackmgr_tx, mut ackmgr_rx) = self::ack_manager::new(config);
        let ackmgr_tx_1 = ackmgr_tx;
        let ackmgr_tx_2 = ackmgr_tx_1.clone();
        let message_sender = MessageSender {
            ackmgr_tx: ackmgr_tx_1.clone(),
        };

        // Create sender thread (1).
        let log1 = log.clone();
        thread::spawn(move || {
            // TODO: proper shutdown mechanism
            loop {
                let packet = ackmgr_rx.fetch();
                let mut buf = Vec::<u8>::new();
                packet.write_to(&mut buf).unwrap();
                log1.log_packet_send(&buf, &packet);

                socket_out.send(&buf).unwrap();
            }
        });

        // Create reader thread (2).
        thread::spawn(move || {
            // TODO: Determine good maximum size. If it's too be big we are wasting memory,
            // if it's too small things will explode.
            //
            // → At first I wanted to make this dynamic but this turned out to not be
            // possible, and maybe it would have been really inefficient. A
            // workaround could be to use our own struct directly reading
            // from a Read and using a larger array as needed?
            let mut packet_log = FifoCache::<SequenceNumber>::new(10000);

            loop {
                // TODO: move back up after debugging
                let mut buf = [0u8; 4096];
                // Read from socket in blocking way.
                let (buf_size, _) = socket_in.recv_from(&mut buf).unwrap();

                // Parse the packet.
                let packet_res = Packet::read(&buf[..buf_size]);
                log.log_packet_recv(&buf[..buf_size], &packet_res);
                let packet = match packet_res {
                    Ok(pkt) => pkt,
                    Err(_) => continue,
                };

                // Read appended acks and send ack if requested (reliable packet).
                {
                    for ack in packet.appended_acks.iter() {
                        ackmgr_tx_1.register_ack(*ack).unwrap();
                    }
                }
                if packet.is_reliable() {
                    {
                        ackmgr_tx_1.send_ack(packet.sequence_number).unwrap();
                    }

                    // Check if we did receive the packet already and the remote just resent it
                    // again anyway.
                    let duplicate = packet_log.contains(&packet.sequence_number);
                    packet_log.insert(packet.sequence_number);
                    if duplicate {
                        continue;
                    }
                }

                match packet.message {
                    MessageInstance::PacketAck(msg) => {
                        // Pass the acks to the ack manager (and don't yield the packet).
                        for packet_ack in msg.packets {
                            ackmgr_tx_1.register_ack(packet_ack.id).unwrap();
                        }
                    }
                    msg => {
                        let _ = message_handlers.handle(msg, &message_sender).map_err(|err| {
                            match err.kind {
                                handlers::ErrorKind::NoHandler => {
                                    // Yield the message to the incoming message channel.
                                    incoming_tx.send(err.msg).unwrap();
                                },
                                _ => {
                                    // TODO: handle the error somewhere
                                    panic!("unhandled error: {:?}", err);
                                }
                            }
                        });
                    }
                }
            }
        });

        Ok(Circuit {
            incoming: incoming_rx,
            ackmgr_tx: ackmgr_tx_2,
        })
    }

    /// Send a message through the circuit.
    ///
    /// This returns a `SendMessage` instance which is a `Future`.
    /// However once you send it using this method you needn't
    /// necessarily poll it for progress to be made. It will be handed over
    /// to the sender threads of this Circuit and you will be able to confirm
    /// it has finished successfully or failed by polling the returned future.
    pub fn send<M: Into<MessageInstance>>(&self, msg: M, reliable: bool) -> SendMessage {
        self.ackmgr_tx.send_msg(msg.into(), reliable)
    }

    /// Reads a message and returns it.
    ///
    /// If there is no message available yet it will block the current thread
    /// until there is one available, or if there is a timeout specified it will
    /// wait at most for the specified duration before returning an error.
    pub fn read(&self, timeout: Option<Duration>) -> Result<MessageInstance, ReadMessageError> {
        match timeout {
            Some(t) => Ok(self.incoming.recv_timeout(t)?),
            None => Ok(self.incoming.recv()?),
        }
    }

    /// Trys reading a message and returns it if one is available right away.
    ///
    /// Otherwise this won't block the current thread and None will be returned.
    pub fn try_read(&self) -> Result<MessageInstance, ReadMessageError> {
        Ok(self.incoming.try_recv()?)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitConfig {
    /// The number of seconds before an unconfirmed packet becomes invalid.
    /// If multiple attempts are allowed, each single attempt will get at most
    /// this time before
    /// timing out.
    pub send_timeout: Duration,

    /// The number of times resending an unacknowledged packet before reporting
    /// it as failure.
    pub send_attempts: usize,
}
