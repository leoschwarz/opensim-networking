//! Implementation of an "adressable queue", that is a FIFO queue where it is possible to directly
//! remove values directly by a key.

// TODO: Improve this, test it thoroughly as it is the core of the AckManager.

use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

struct Item<K, V> {
    key: K,
    val: Mutex<Option<V>>,
}

pub struct Queue<K, V> {
    items: VecDeque<Arc<Item<K, V>>>,
    pointers: HashMap<K, Arc<Item<K, V>>>,
}

impl<K, V> Queue<K, V>
where
    K: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        Queue {
            items: VecDeque::new(),
            pointers: HashMap::new(),
        }
    }

    /// Insert an entry at the end of the queue.
    pub fn insert(&mut self, key: K, value: V) {
        let arc = Arc::new(Item {
            key: key.clone(),
            val: Mutex::new(Some(value)),
        });
        self.items.push_back(Arc::clone(&arc));
        self.pointers.insert(key, arc);
    }

    /// Insert an entry at the beginning of the queue.
    ///
    /// This is mostly useful when removing the head and
    /// then deciding to put it back into the queue.
    pub fn insert_head(&mut self, key: K, value: V) {
        let arc = Arc::new(Item {
            key: key.clone(),
            val: Mutex::new(Some(value)),
        });
        self.items.push_front(Arc::clone(&arc));
        self.pointers.insert(key, arc);
    }

    pub fn remove_key(&mut self, key: &K) -> Option<V> {
        if let Some(item) = self.pointers.remove(key) {
            let mut val = None;
            ::std::mem::swap(&mut val, &mut *item.val.lock().unwrap());
            return val;
        }
        None
    }

    pub fn remove_head(&mut self) -> Option<V> {
        while let Some(item) = self.items.pop_front() {
            let is_some = item.val.lock().unwrap().is_some();
            if is_some {
                self.pointers.remove(&item.key);
                return Arc::try_unwrap(item)
                    .ok()
                    .unwrap()
                    .val
                    .into_inner()
                    .unwrap();
            }
        }
        None
    }
}
