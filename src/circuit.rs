use login::LoginResponse;
use messages::MessageInstance;
use packet::{Packet, SequenceNumber};
use util::mpsc_read_many;

use std::collections::HashMap;
use std::io::Error as IoError;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc;
use std::time::Duration;
use std::thread;

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
    message: MessageInstance,
    reliable: bool,
    attempts: u8,
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
struct MessageManager {
    incoming: mpsc::Receiver<MessageInstance>,
    outgoing: mpsc::Sender<MessageManagerItem>,
}

impl MessageManager {
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
            let mut sequence_number = 0u32;

            loop {
                // Blocking read of the next outgoing item.
                match outgoing_rx.recv() {
                    Ok(item) => {
                        sequence_number += 1;

                        // Create the packet instance.
                        let mut packet = Packet::new(item.message, sequence_number);
                        packet.appended_acks = mpsc_read_many(&acks_outgoing_rx, 255);

                        // Send through the socket.
                        let mut buf = Vec::<u8>::new();
                        packet.write_to(&mut buf);
                        socket_out.send_to(&buf, sim_address);

                        // Register pending ack if packet is to be sent reliably.
                        if packet.is_reliable() {
                            // Create new item.
                            let wait_item = MessageManagerItem {
                                message: packet.message,
                                reliable: true,
                                attempts: item.attempts + 1,
                            };

                            // Register pending ack.
                            register_packet_tx.send(wait_item);
                        }
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
        thread::spawn(move ||
            loop {
                let item = register_packet_rx.recv().unwrap();

            }
        });


        Ok(MessageManager {
               incoming: incoming_rx,
               outgoing: outgoing_tx,
           })
    }
}
