use std::collections::BinaryHeap;
use time::{Duration, Timespec};

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
        Self {
            queue: BinaryHeap::new()
        }
    }

    /// Insert an item into the backoff queue.
    /// `item`: The item to be inserted.
    /// `wait_s`: The number of seconds to wailt at least before retrieving the item.
    pub insert(&mut self, item: T, wait_s: f64) {
        let secs = wait_s.floor() as i64;
        let nsecs = ((wait_s % 1.) * 1e9) as i32;

        self.queue.push(BackoffQueueItem{
            value: item,
            wait_until: time::get_time() + Duration{ sec: secs, nsec: nsecs }
        });
    }

    /// Extract an item if one is available.
    pub extract(&mut self) -> Option<T> {
        match self.state() {
            BackoffQueueState::ItemReady => self.queue.pop(),
            _ => None
        }
    }

    /// Get the current state of the queue.
    pub state(&self) -> BackoffQueueState {
        match self.queue.peek() {
            None => BackoffQueueState::Empty,
            Some(ref item_ref) => {
                // Check if enough time has passed.
                let now = time::get_time();
                if now >= item_ref.wait_until {
                    BackoffQueueState::ItemReady
                } else {
                    BackoffQueueState::Wait(item_ref.wait_until - now)
                }
            }
        }
    }

    /// Return true if the queue is empty.
    pub is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

struct BackoffQueueItem<T> {
    /// The value stored in this item.
    value: T,
    /// Until which time we have to wait before this could this item be retrieved.
    wait_until: Timespec
}

impl<T> Ord for BackoffQueueItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Note that we have to reverse the ordering since Rust's BinaryHeap
        // is a maximum heap even though we want a minimum one.
        self.wait_until.cmp(&other.wait_until).reverse()
    }
}
