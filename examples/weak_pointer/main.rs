use std::cell::UnsafeCell;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::fence;
use std::{ops::Deref, ptr::NonNull, sync::atomic::AtomicUsize};

struct ArcData<T> {
    arc_ref_count: AtomicUsize,
    alloc_ref_count: AtomicUsize,
    data: UnsafeCell<Option<T>>,
}

struct Arc<T> {
    weak: Weak<T>,
}

struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

impl<T> Weak<T> {
    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut n = self.data().arc_ref_count.load(Relaxed);
        loop {
            if n == 0 {
                return None;
            }
            if let Err(e) =
                self.data()
                    .arc_ref_count
                    .compare_exchange_weak(n, n + 1, Relaxed, Relaxed)
            {
                n = e;
                continue;
            }
            return Some(Arc { weak: self.clone() });
        }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        // TODO: handle overflow
        self.data().alloc_ref_count.fetch_add(1, Relaxed);
        Weak { ptr: self.ptr }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        // TODO: memory ordering
        if self.data().alloc_ref_count.fetch_sub(1, Release) == 1 {
            fence(Acquire);
            unsafe { drop(Box::from_raw(self.ptr.as_ptr())) }
        }
    }
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Arc<T> {
        Arc {
            weak: Weak {
                ptr: NonNull::from(Box::leak(Box::new(ArcData {
                    arc_ref_count: AtomicUsize::new(1),
                    alloc_ref_count: AtomicUsize::new(1),
                    data: UnsafeCell::new(Some(data)),
                }))),
            },
        }
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.weak.data().alloc_ref_count.load(Relaxed) == 1 {
            fence(Acquire);
            let arc_data = unsafe { arc.weak.ptr.as_mut() };
            let option = arc_data.data.get_mut();
            let data = option.as_mut().unwrap();
            Some(data)
        } else {
            None
        }
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        arc.weak.clone()
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let ptr = self.weak.data().data.get();
        unsafe { (*ptr).as_ref().unwrap() }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        // TODO: handle overflow
        let weak = self.weak.clone();
        weak.data().arc_ref_count.fetch_add(1, Relaxed);
        Arc { weak }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.weak.data().arc_ref_count.fetch_sub(1, Relaxed) == 1 {
            fence(Acquire);
            let ptr = self.weak.data().data.get();
            unsafe {
                (*ptr) = None;
            }
        }
    }
}

fn main() {}

#[test]
fn test() {
    static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);

    struct DetectDrop;

    impl Drop for DetectDrop {
        fn drop(&mut self) {
            NUM_DROPS.fetch_add(1, Relaxed);
        }
    }

    let x = Arc::new(("hello", DetectDrop));
    let y = Arc::downgrade(&x);
    let z = Arc::downgrade(&x);

    let t = std::thread::spawn(move || {
        let y = y.upgrade().unwrap();
        assert_eq!(y.0, "hello");
    });

    assert_eq!(x.0, "hello");

    t.join().unwrap();

    assert_eq!(NUM_DROPS.load(Relaxed), 0);
    assert!(z.upgrade().is_some());

    drop(x);

    assert_eq!(NUM_DROPS.load(Relaxed), 1);
    assert!(z.upgrade().is_none());
}
