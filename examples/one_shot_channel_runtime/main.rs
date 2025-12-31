use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;
use std::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::AtomicBool};

struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    in_use: AtomicBool,
    ready: AtomicBool,
}

unsafe impl<T: Send> Sync for Channel<T> {}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            in_use: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }

    // only call this once
    pub fn send(&self, message: T) {
        if self.in_use.swap(true, Relaxed) {
            panic!("can't send more than one message");
        }
        unsafe {
            (*self.message.get()).write(message);
        }
        self.ready.store(true, Release)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Relaxed)
    }

    pub fn receive(&self) -> T {
        if !self.ready.swap(false, Acquire) {
            panic!("no message available");
        }
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

fn main() {
    let channel = Channel::new();
    let t = thread::current();
    thread::scope(|s| {
        s.spawn(|| {
            channel.send("hello, world!");
            // channel.send("hello, world!");
            // thread '<unnamed>' panicked at examples/one_shot_channel/main.rs:25:13:
            // can't send more than one message
            t.unpark();
        });
        while !channel.is_ready() {
            thread::park();
        }
        assert_eq!(channel.receive(), "hello, world!");
    })
}
