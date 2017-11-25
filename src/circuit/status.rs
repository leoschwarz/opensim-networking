//! Contains the status types used for the circuit module.

use types::SequenceNumber;

use futures::{self, Async, Future, Poll};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

/// The `Future` returned by the `Circuit`'s send method.
#[derive(Debug)]
pub struct SendMessage {
    status: Arc<RwLock<SendMessageStatus>>,
    task: Arc<Mutex<Option<futures::task::Task>>>,
}

impl SendMessage {
    pub(crate) fn clone(&self) -> Self {
        SendMessage {
            status: Arc::clone(&self.status),
            task: Arc::clone(&self.task),
        }
    }

    pub(crate) fn new(status: SendMessageStatus) -> Self {
        SendMessage {
            status: Arc::new(RwLock::new(status)),
            task: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn update_status(&mut self, new_status: SendMessageStatus) {
        *self.status.write().unwrap() = new_status;

        let task = self.task.lock().unwrap();
        match *task {
            Some(ref t) => t.notify(),
            None => {}
        }
    }

    pub(crate) fn get_status(&self) -> SendMessageStatus {
        self.status.read().unwrap().clone()
    }
}

impl Future for SendMessage {
    /// If we complete without error it means an unreliable packet was sent or a reliable
    /// packet was sent and ack'ed by the server.
    type Item = ();
    type Error = SendMessageError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match *self.status.read().unwrap() {
            SendMessageStatus::PendingSend { .. } | SendMessageStatus::PendingAck { .. } => {
                let mut task = self.task.lock().unwrap();
                if task.is_none() {
                    *task = Some(futures::task::current());
                }
                Ok(Async::NotReady)
            }
            SendMessageStatus::Success => Ok(Async::Ready(())),
            SendMessageStatus::Failure(err) => Err(err),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SendMessageError {
    /// Remote failed to acknowledge the packet.
    FailedAck,
}

/// Describes the current status of a message that was submitted to be sent.
#[derive(Debug, Clone, Copy)]
pub enum SendMessageStatus {
    /// Has not been sent through the socket yet.
    PendingSend { reliable: bool },
    /// Has been sent but not acknowledged yet.
    /// The attempt variant describes the number of attempts already made.
    /// (0 → this is the first attempt, 1 → 2nd attempt, etc.)
    /// timeout: holds the time after which the current attemt is considered timed out.
    PendingAck {
        attempt: u8,
        timeout: Instant,
        id: SequenceNumber,
    },
    /// Has finished successfully.
    Success,
    /// Has failed.
    Failure(SendMessageError),
}

impl SendMessageStatus {
    pub fn is_failure(&self) -> bool {
        match *self {
            SendMessageStatus::Failure(_) => true,
            _ => false,
        }
    }
}
