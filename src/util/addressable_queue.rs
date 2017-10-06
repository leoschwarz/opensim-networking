//! Implementation of an "adressable queue", that is a FIFO queue where it is possible to directly
//! extract names by a key.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::hash::Hash;

struct Item<K, V> {
    key: K,
    val: RefCell<Option<V>>,
}

pub struct Queue<K, V> {
    items: VecDeque<Rc<Item<K, V>>>,
    pointers: HashMap<K, Rc<Item<K, V>>>,
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

    pub fn insert(&mut self, key: K, value: V) {
        let rc = Rc::new(Item {
            key: key.clone(),
            val: RefCell::new(Some(value)),
        });
        self.items.push_back(rc.clone());
        self.pointers.insert(key, rc);
    }

    pub fn remove_key(&mut self, key: &K) -> Option<V> {
        if let Some(item) = self.pointers.remove(key) {
            let mut val = None;
            std::mem::swap(&mut val, &mut *item.val.borrow_mut());
            return val;
        }
        None
    }

    pub fn remove_head(&mut self) -> Option<V> {
        while let Some(item) = self.items.pop_front() {
            let is_some = {item.val.borrow().is_some()};
            if is_some {
                self.pointers.remove(&item.key);
                return Rc::try_unwrap(item).ok().unwrap().val.into_inner();
            }
        }
        None
    }
}