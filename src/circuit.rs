use login::LoginResponse;
use messages::MessageInstance;
use packet::{Packet, SequenceNumber, PACKET_RESENT};
use util::{AtomicU32Counter, BackoffQueue, BackoffQueueState, FifoCache, mpsc_read_many};

use futures::{Async, Future, Poll};
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
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse, config: CircuitConfig) -> Result<Circuit, IoError> {
        let sim_address = SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip, login_res.sim_port));
        Ok(Circuit { message_manager: MessageManager::start(sim_address, config)? })
    }

    /// Send a message through the circuit.
    ///
    /// This returns a `SendMessage` instance which is a `Future`. However once you send it using
    /// this method you needn't necessarily poll it for progress to be made. It will be handed over
    /// to the sender threads of this Circuit and you will be able to confirm it has finished
    /// successfully or failed by polling the returned future.
    pub fn send<M: Into<MessageInstance>>(&self, msg: M, reliable: bool) -> SendMessage {
        self.message_manager.send_message(msg.into(), reliable)
    }

    /// Reads a message and returns it.
    /// If there is no message available yet it will block the current thread until there is one
    /// available.
    pub fn read(&self) -> MessageInstance {
        self.message_manager
            .incoming
            .recv()
            .unwrap()
    }

    /// Trys to read a message and returns it if one is available right away. Otherwise this won't
    /// block the current thread and None will be returned.
    pub fn try_read(&self) -> Option<MessageInstance> {
        self.message_manager
            .incoming
            .try_recv()
            .ok()
    }
}

#[derive(Debug, Clone)]
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
            SendMessageStatus::PendingAck { attempt, .. } => {
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

enum AckManagerOutput {
    /// The enclosed item was extracted from the manager.
    Item(MessageManagerItem),
    /// Wait at least until the specified time.
    Wait(Duration),
}

/// Stores MessageManager items until it is checked, whether the items have been ack'ed.
struct AckManager {
    queue: BackoffQueue<MessageManagerItem>,
    min_wait: Duration,
}

impl AckManager {
    fn new(config: &CircuitConfig) -> AckManager {
        /// TODO: Update this to the minimum wait time if we allow in the future to have per packet
        /// timeout durations.
        let min_wait = config.send_timeout;
        AckManager {
            queue: BackoffQueue::new(),
            min_wait: min_wait,
        }
    }

    fn insert(&mut self, item: MessageManagerItem) {
        let status = item.status
            .lock()
            .unwrap()
            .clone();

        match status {
            SendMessageStatus::PendingAck { timeout, .. } => {
                self.queue.insert(item, timeout);
            }
            _ => panic!("Contract violation!"),
        }
    }

    fn fetch(&mut self) -> AckManagerOutput {
        match self.queue.state() {
            BackoffQueueState::ItemReady => {
                let item = self.queue.extract().unwrap();
                AckManagerOutput::Item(item)
            }
            BackoffQueueState::Wait(duration) => AckManagerOutput::Wait(duration),
            BackoffQueueState::Empty => AckManagerOutput::Wait(self.min_wait),
        }
    }
}

/// Handles message management and sending and receiving of packages through the socket.
///
/// Note that this is a rather expensive struct creating multiple threads and communication
/// channels.
///
/// TODO:
/// - Threadsafe API? Reader/Writer objects?
/// - Stop/exit functionality.
struct MessageManager {
    incoming: mpsc::Receiver<MessageInstance>,
    outgoing: mpsc::Sender<MessageManagerItem>,
    sequence_counter: AtomicU32Counter,
}

impl MessageManager {
    fn send_message(&self, msg: MessageInstance, reliable: bool) -> SendMessage {
        let mut packet = Packet::new(msg, self.sequence_counter.next());
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
        let (incoming_tx, incoming_rx) = mpsc::channel::<MessageInstance>();
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<MessageManagerItem>();
        let (acks_outgoing_tx, acks_outgoing_rx) = mpsc::channel::<SequenceNumber>();
        let (acks_incoming_tx, acks_incoming_rx) = mpsc::channel::<SequenceNumber>();
        let (register_packet_tx, register_packet_rx) = mpsc::channel::<MessageManagerItem>();
        let outgoing_tx2 = outgoing_tx.clone();

        // Create sockets.
        let socket_out = UdpSocket::bind("0.0.0.0:0")?;
        socket_out.set_read_timeout(None)?;
        socket_out.set_nonblocking(false)?;
        let socket_in = socket_out.try_clone()?;

        // Create sender thread.
        let config1 = config.clone();
        thread::spawn(move || {
            loop {
                // Blocking read of the next outgoing item.
                match outgoing_rx.recv() {
                    Ok(item) => {
                        let (mut packet, status) = (item.packet, item.status);
                        // Append pending acks.
                        if packet.appended_acks.is_empty() {
                            packet.appended_acks = mpsc_read_many(&acks_outgoing_rx, 255);
                        }

                        // Determine the next status.
                        let next_status =
                            status.lock().unwrap().next_status(packet.is_reliable(), &config1);

                        // Send if there isn't a failure (i.e. too many attempts resending the
                        // packet).
                        if !next_status.is_failure() {
                            // Mark the packet as resend if it is a resend.
                            if next_status.is_resend() {
                                packet.enable_flags(PACKET_RESENT);
                            }

                            let mut buf = Vec::<u8>::new();
                            packet.write_to(&mut buf).unwrap();
                            socket_out.send_to(&buf, sim_address).unwrap();
                        }

                        // Update item's status.
                        *status.lock().unwrap() = next_status;

                        if packet.is_reliable() {
                            // Register the packet for timeout checking.
                            register_packet_tx.send(MessageManagerItem {
                                                        packet: packet,
                                                        status: status,
                                                    })
                                .unwrap();
                        }
                    }
                    Err(_) => panic!("channel closed"),
                }
            }
        });

        // Create reader thread.
        thread::spawn(move || {
            let mut buf = Vec::<u8>::new();
            let mut packet_log = FifoCache::<SequenceNumber>::new(200_000);

            loop {
                // Read from socket in blocking way.
                buf.clear();
                socket_in.recv(&mut buf).unwrap();

                // Parse the packet.
                let packet = Packet::read(&buf).unwrap();

                // Read appended acks and send ack if requested (reliable packet).
                for ack in &packet.appended_acks {
                    acks_incoming_tx.send(*ack).unwrap();
                }
                if packet.is_reliable() {
                    acks_outgoing_tx.send(packet.sequence_number).unwrap();

                    // Check if we did receive the packet already and the remote just resent it
                    // again anyway.
                    let duplicate = packet_log.contains(&packet.sequence_number);
                    packet_log.insert(packet.sequence_number);
                    if duplicate {
                        continue;
                    }
                }

                // Yield the received message.
                incoming_tx.send(packet.message).unwrap();
            }
        });

        // Create ack book-keeping thread.
        thread::spawn(move || {
            let mut ack_manager = AckManager::new(&config);
            let mut ack_list = FifoCache::<SequenceNumber>::new(200_000);

            loop {
                // Fetch the next item from the AckManager.
                //
                // If none is available, wait for the time specified by the AckManager.
                // Notice that even if it is empty we would have to wait for that amount
                // of time at least until one of the pending packets would time out, so
                // nothing bad should happen by waiting some longer.
                let item = match ack_manager.fetch() {
                    AckManagerOutput::Item(item) => Some(item),
                    AckManagerOutput::Wait(duration) => {
                        thread::sleep(duration.to_std().unwrap());
                        None
                    }
                };

                // Fetch all available incoming acks from acks_incoming and check if any
                // of these match our item's packet.
                while let Ok(ack) = acks_incoming_rx.try_recv() {
                    ack_list.insert(ack);
                }

                // Check the current item.
                item.map(|it| {
                    let sequence_number = it.packet.sequence_number;
                    if ack_list.contains(&sequence_number) {
                        // Mark item as successful and don't do anything else.
                        *it.status.lock().unwrap() = SendMessageStatus::Success;
                    } else {
                        // Pass the item again into the sending queue, the handler
                        // there will take care of updating its status too.
                        outgoing_tx2.send(it).unwrap();
                    }
                });

                // Put incoming items into the ack manager.
                while let Ok(item) = register_packet_rx.try_recv() {
                    ack_manager.insert(item);
                }
            }
        });

        Ok(MessageManager {
               incoming: incoming_rx,
               outgoing: outgoing_tx,
               sequence_counter: AtomicU32Counter::new(0),
           })
    }
}
