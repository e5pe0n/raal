use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

struct Channel<T> {
    queue: Mutex<VecDeque<T>>,
    item_ready: Condvar,
}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            item_ready: Condvar::new(),
        }
    }

    pub fn send(&self, message: T) {
        let mut g = self.queue.lock().unwrap();
        g.push_back(message);
        self.item_ready.notify_one();
    }

    pub fn receive(&self) -> T {
        let mut g = self.queue.lock().unwrap();
        loop {
            if let Some(message) = g.pop_front() {
                return message;
            }
            g = self.item_ready.wait(g).unwrap();
        }
    }
}

fn main() {}
