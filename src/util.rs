use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use time::{Duration, Timespec, get_time};
use std::sync::mpsc;

/// read elements from the reader in a non-blocking (async) fashion.
/// we try to read up to max_count elements if they are available, but if they
/// aren't we'll just return as much as possible. (possibly even empty vector.)
pub fn mpsc_read_many<T>(recv: &mpsc::Receiver<T>, max_count: usize) -> Vec<T>
{
    let mut res = Vec::new();

    while res.len() < max_count {
        match recv.try_recv() {
            Ok(item) => res.push(item),
            Err(_) => return res
        }
    }

    res
}

/// A BackoffQueue provides a time-based interface over a priority queue.
/// Items can be inserted specifying a minimum amount of time that has to elapse
/// before they can be retrieved from the queue.
pub struct BackoffQueue<T> {
    queue: BinaryHeap<BackoffQueueItem<T>>
}

/// Indicates the current state of a BackoffQueue.
pub enum BackoffQueueState {
    /// There is an item available which can be read now.
    ItemReady,

    /// The queue is empty.
    Empty,

    /// An item will be available, only after waiting for the specified
    /// amount of time.
    Wait(Duration)
}

impl<T> BackoffQueue<T> {
    pub fn new() -> Self {
        BackoffQueue {
            queue: BinaryHeap::new()
        }
    }

    /// Insert an item into the backoff queue.
    /// `item`: The item to be inserted.
    /// `wait`: The duration to wait at least before retrieving the item.
    pub fn insert(&mut self, item: T, wait: Duration) {
        self.queue.push(BackoffQueueItem{
            value: item,
            wait_until: ::time::get_time() + wait
        });
    }

    /// Extract an item if one is available.
    pub fn extract(&mut self) -> Option<T> {
        match self.state() {
            BackoffQueueState::ItemReady => Some(self.queue.pop().unwrap().value),
            _ => None
        }
    }

    /// Get the current state of the queue.
    pub fn state(&self) -> BackoffQueueState {
        match self.queue.peek() {
            None => BackoffQueueState::Empty,
            Some(ref item_ref) => {
                // Check if enough time has passed.
                let now = ::time::get_time();
                if now >= item_ref.wait_until {
                    BackoffQueueState::ItemReady
                } else {
                    BackoffQueueState::Wait(item_ref.wait_until - now)
                }
            }
        }
    }

    /// Return true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

struct BackoffQueueItem<T> {
    /// The value stored in this item.
    value: T,
    /// Until which time we have to wait before this could this item be retrieved.
    wait_until: Timespec
}

impl<T> PartialEq for BackoffQueueItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.wait_until == other.wait_until
    }
}

impl<T> Eq for BackoffQueueItem<T> {
}

impl<T> PartialOrd for BackoffQueueItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for BackoffQueueItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Note that we have to reverse the ordering since Rust's BinaryHeap
        // is a maximum heap even though we want a minimum one.
        self.wait_until.cmp(&other.wait_until).reverse()
    }
}

