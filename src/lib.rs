#![feature(unsafe_destructor)]
#![feature(core, std_misc, io, test)]
#![deny(warnings)]

extern crate time;
extern crate test;

use std::mem;
use std::ops::{Deref, DerefMut};
use std::cell::UnsafeCell;
use std::sync::{Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod tests;

/// Reader-writer lock with writer priority and optimistic reads.
pub struct Seqloq<T> {
    mutex: Mutex<()>,
    seqnum: AtomicUsize,
    data: UnsafeCell<T>,
}

#[inline(always)]
fn non_atomic_increment(x: &AtomicUsize) {
    let v = x.load(Ordering::SeqCst);
    x.store(v+1, Ordering::SeqCst);
}

unsafe impl<T: Send> Send for Seqloq<T> { }
unsafe impl<T: Send> Sync for Seqloq<T> { }

/// Represents exclusive, read/write access.
pub struct SeqloqGuard<'a, T: 'a> {
    seqloq: &'a Seqloq<T>,
    #[allow(dead_code)] guard: MutexGuard<'a, ()>,
    ptr: *mut T,
}

impl<T> Seqloq<T>
    where T: Send + Copy,
{
    #[inline]
    pub fn new(t: T) -> Seqloq<T> {
        Seqloq {
            mutex: Mutex::new(()),
            seqnum: AtomicUsize::new(0),
            data: UnsafeCell::new(t),
        }
    }

    /// Peek at the data without locking it.
    ///
    /// The pointed-to data can change at any time!  In that case the
    /// callback's return value may be meaningless (it will be destroyed)
    /// but the callback must not violate memory safety.  The `Send + Copy`
    /// bound limits somewhat the damage that can be done, but there may be
    /// lurking soundness issues.
    ///
    /// The callback will run more than once, if a concurrent write occurs.
    #[inline]
    pub fn peek<F, R>(&self, mut f: F) -> R
        where F: FnMut(*const T) -> R,
    {
        loop {
            let old = self.seqnum.load(Ordering::SeqCst);
            if (old & 1) != 0 {
                // A writer is active.
                // FIXME: smarter spinlocking
                continue;
            }

            let res = f(self.data.get());

            let new = self.seqnum.load(Ordering::SeqCst);
            if new == old {
                return res;
            }
        }
    }

    /// Read the data without locking.
    ///
    /// Unlike `peek`, this involves a copy.  But it's safe, and it's sometimes
    /// just as fast as `peek`.
    #[inline]
    pub fn read(&self) -> T {
        self.peek(|x| unsafe { *x })
    }

    /// Lock for exclusive, read/write access.
    ///
    /// Readers will see changes, but will automatically re-try until they have
    /// a consistent view.
    #[inline]
    pub fn lock<'a>(&'a self) -> SeqloqGuard<'a, T> {
        let guard = self.mutex.lock().unwrap();
        non_atomic_increment(&self.seqnum);
        SeqloqGuard {
            seqloq: self,
            guard: guard,
            ptr: self.data.get(),
        }
    }
}

impl<'a, T> Deref for SeqloqGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref<'b>(&'b self) -> &'b T {
        unsafe { mem::transmute(self.ptr) }
    }
}

impl<'a, T> DerefMut for SeqloqGuard<'a, T> {
    #[inline]
    fn deref_mut<'b>(&'b mut self) -> &'b mut T {
        unsafe { mem::transmute(self.ptr) }
    }
}

#[unsafe_destructor]
impl<'a, T> Drop for SeqloqGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        non_atomic_increment(&self.seqloq.seqnum);
    }
}

#[test]
fn smoke_test() {
    let x: Seqloq<u32> = Seqloq::new(3);
    assert_eq!(x.peek(|v| unsafe { *v }), 3);

    {
        let mut g = x.lock();
        assert_eq!(*g, 3);
        *g = 4;
        assert_eq!(*g, 4);
    }

    assert_eq!(x.read(), 4);
}

#[test]
fn traits() {
    fn check<T: Send + Sync>(_: &T) { }
    check(&Seqloq::new('x'));
}
