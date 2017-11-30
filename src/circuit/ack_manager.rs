//! Encapsulates the logic for handling incoming and outgoing packet acks.

// Description of actions and events, for a potential future event based
// implementation
//
// - packet out
//   → Socket write
//   → Register ack timeout
// - packet in
//   → Retrieve acks and remove pending timeouts
// TODO: This is currently the biggest problem how would I implement this
// using Rust futures?
//   → Queue the packet for retrieval by client application
// - ack out
// → register ack for timeout queue, either append it to the next packet or
// send it in a     dedicated packet if waiting for too long.

use circuit::{CircuitConfig, SendMessage, SendMessageError, SendMessageStatus};
use packet::{Packet, PacketFlags};
use types::SequenceNumber;
use util::{mpsc_read_many, AtomicU32Counter};
use util::addressable_queue::Queue as AddressableQueue;
use messages::{MessageInstance, PacketAck, PacketAck_Packets};

use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};
use std::thread;

pub struct AckManagerRx {
    /// Messages waiting for their confirmation.
    acks_wait: AddressableQueue<SequenceNumber, PendingMessage>,

    /// Acks we have received but not processed yet.
    acks_inc: mpsc::Receiver<SequenceNumber>,

    /// Acks to be sent out.
    acks_out: mpsc::Receiver<SequenceNumber>,

    /// Messages waiting to be sent out.
    msgs_out: mpsc::Receiver<PendingMessage>,

    /// Copy of circuit config.
    config: CircuitConfig,
    sequence_counter: AtomicU32Counter,
}

impl AckManagerRx {
    fn _fetch_loop(&mut self) -> (Packet, SendMessage) {
        loop {
            if let Some(pending_msg) = self._next_message() {
                // Create packet instance and update status.
                let mut future = pending_msg.future.clone();
                let (packet, new_status) = self._prepare_packet(pending_msg);
                future.update_status(new_status);

                if let Some(mut packet) = packet {
                    // Append some pending acks if there are any.
                    let mut acks = mpsc_read_many(&self.acks_out, 255);
                    packet.appended_acks.append(&mut acks);

                    // Return the packet to be sent.
                    return (packet, future);
                }
            } else {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// Returns the next packet to be sent to the sim.
    ///
    /// Note that this method will block the current thread until something is
    /// available.
    pub fn fetch(&mut self) -> Packet {
        let (packet, future) = self._fetch_loop();

        if packet.is_reliable() {
            // Put message into wait queue.
            self.acks_wait.insert(
                packet.sequence_number,
                PendingMessage {
                    message: packet.message.clone(),
                    future: future,
                },
            );
        }

        packet
    }

    fn _next_message(&mut self) -> Option<PendingMessage> {
        // Apply all available incoming acks.
        while let Ok(ack) = self.acks_inc.try_recv() {
            if let Some(mut acked_msg) = self.acks_wait.remove_key(&ack) {
                // debug!(self.logger, "incoming ack (msg found): {}", ack);

                // Mark the PendingMessage as successful.
                acked_msg.future.update_status(SendMessageStatus::Success);
            } else {
                // debug!(self.logger, "incoming ack (msg not found): {}", ack);
            }
        }

        // First check if there is a message waiting too long for an ack already, then
        // check the other pending messages for sending.
        if let Some(wait_oldest) = self.acks_wait.remove_head() {
            if wait_oldest.is_too_old() {
                return Some(wait_oldest);
            } else {
                let seq_number = wait_oldest.sequence_number().unwrap();
                self.acks_wait.insert_head(seq_number, wait_oldest);
            }
        }

        match self.msgs_out.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => {
                // If there are pending acks to be sent out, return a PacketAck.
                let acks = mpsc_read_many(&self.acks_out, 255);
                if acks.is_empty() {
                    None
                } else {
                    Some(PendingMessage {
                        message: PacketAck {
                            packets: acks.iter()
                                .map(|num| PacketAck_Packets { id: *num })
                                .collect(),
                        }.into(),
                        future: SendMessage::new(
                            SendMessageStatus::PendingSend { reliable: false },
                        ),
                    })
                }
            }
            // TODO: return error.
            Err(TryRecvError::Disconnected) => panic!("unimplemented error handling"),
        }
    }

    fn _prepare_packet(&self, msg: PendingMessage) -> (Option<Packet>, SendMessageStatus) {
        let old_status = msg.future.get_status();
        match old_status {
            SendMessageStatus::PendingSend { reliable } => {
                let seq_num = self.sequence_counter.next();
                let new_status = SendMessageStatus::PendingAck {
                    attempt: 0,
                    timeout: Instant::now() + self.config.send_timeout,
                    id: seq_num,
                };

                let mut packet = Packet::new(msg.message, seq_num);
                packet.set_reliable(reliable);
                (Some(packet), new_status)
            }
            SendMessageStatus::PendingAck { attempt, id, .. } => {
                let attempt = attempt + 1;
                if attempt >= self.config.send_attempts as u8 {
                    (
                        None,
                        SendMessageStatus::Failure(SendMessageError::FailedAck),
                    )
                } else {
                    let new_status = SendMessageStatus::PendingAck {
                        attempt: attempt,
                        timeout: Instant::now() + self.config.send_timeout,
                        id: id,
                    };
                    let mut packet = Packet::new(msg.message, id);
                    packet.set_reliable(true);
                    packet.enable_flags(PacketFlags::RESENT);
                    (Some(packet), new_status)
                }
            }
            SendMessageStatus::Success | SendMessageStatus::Failure(_) => (None, old_status),
        }
    }
}

#[derive(Clone)]
pub struct AckManagerTx {
    acks_out: mpsc::Sender<SequenceNumber>,
    acks_inc: mpsc::Sender<SequenceNumber>,
    msgs_out: mpsc::Sender<PendingMessage>,

    /// Copy of circuit config.
    config: CircuitConfig,
}

impl AckManagerTx {
    /// Queue an ack to be sent out as soon as possible.
    pub fn send_ack(&self, ack: SequenceNumber) -> Result<(), mpsc::SendError<SequenceNumber>> {
        // debug!(self.logger, "send_ack: {}", ack);
        self.acks_out.send(ack)
    }

    /// Register an incoming ack to be processed.
    pub fn register_ack(&self, ack: SequenceNumber) -> Result<(), mpsc::SendError<SequenceNumber>> {
        // debug!(self.logger, "register_ack: {}", ack);
        self.acks_inc.send(ack)
    }

    pub fn send_msg(&self, msg: MessageInstance, reliable: bool) -> SendMessage {
        // debug!(self.logger, "send_msg: {:?}", msg);
        let future = SendMessage::new(SendMessageStatus::PendingSend { reliable: reliable });
        let p_m = PendingMessage {
            message: msg,
            future: future.clone(),
        };

        self.msgs_out.send(p_m).unwrap();
        future
    }
}

/// Create a new instance of the AckManager tx and rx.
pub fn new(config: CircuitConfig) -> (AckManagerTx, AckManagerRx) {
    let (acks_out_tx, acks_out_rx) = mpsc::channel();
    let (acks_inc_tx, acks_inc_rx) = mpsc::channel();
    let (msgs_out_tx, msgs_out_rx) = mpsc::channel();

    let tx = AckManagerTx {
        acks_out: acks_out_tx,
        acks_inc: acks_inc_tx,
        msgs_out: msgs_out_tx,
        config: config.clone(),
    };
    let rx = AckManagerRx {
        acks_wait: AddressableQueue::new(),
        acks_inc: acks_inc_rx,
        acks_out: acks_out_rx,
        msgs_out: msgs_out_rx,
        config: config,
        sequence_counter: AtomicU32Counter::new(0),
    };

    (tx, rx)
}

#[derive(Debug)]
pub struct PendingMessage {
    pub message: MessageInstance,
    pub future: SendMessage,
}

impl PendingMessage {
    /// Returns true if the packet has been waiting for an ack for too long by
    /// now.
    fn is_too_old(&self) -> bool {
        match self.future.get_status() {
            SendMessageStatus::PendingAck { timeout, .. } => timeout < Instant::now(),
            _ => false,
        }
    }

    fn sequence_number(&self) -> Option<SequenceNumber> {
        match self.future.get_status() {
            SendMessageStatus::PendingAck { id, .. } => Some(id),
            _ => None,
        }
    }
}
