//! Tests and test infrastructure.
//!
//! Unless you're writing custom benchmarks, you don't need this.

use std::io::timer;
use std::time::Duration;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::cell::UnsafeCell;
use std::thread::Thread;
use std::default::Default;
use std::any::Any;
use time::precise_time_ns;
use test::black_box;

use Seqloq;

#[doc(hidden)]
pub trait TestableMutex: Send + Sync {
    fn create() -> Self;
    fn check(&self, delay: Duration) -> usize;
    fn frob(&self, delay: Duration);
}

const ARRAY_LEN: usize = 4;

#[derive(Copy)]
pub struct TestArray(pub [u64; ARRAY_LEN]);

impl TestArray {
    pub fn new() -> TestArray {
        TestArray([0; ARRAY_LEN])
    }

    pub fn check(&self, delay: Duration) -> usize {
        let v = self.0[0];
        let n = self.0.iter().skip(1)
            .filter(|e| {
                timer::sleep(delay);
                **e != v
            }).count();

        // Make sure it won't short-circuit even if inlined
        black_box(n);
        n
    }

    pub fn frob(&mut self, delay: Duration) {
        for e in self.0.iter_mut() {
            *e += 1;
            timer::sleep(delay);
        }
    }
}

impl TestableMutex for Mutex<TestArray> {
    fn create() -> Mutex<TestArray> {
        Mutex::new(TestArray::new())
    }

    fn check(&self, delay: Duration) -> usize {
        self.lock().unwrap().check(delay)
    }

    fn frob(&self, delay: Duration) {
        self.lock().unwrap().frob(delay);
    }
}

impl TestableMutex for RwLock<TestArray> {
    fn create() -> RwLock<TestArray> {
        RwLock::new(TestArray::new())
    }

    fn check(&self, delay: Duration) -> usize {
        self.read().unwrap().check(delay)
    }

    fn frob(&self, delay: Duration) {
        self.write().unwrap().frob(delay);
    }
}

impl TestableMutex for Seqloq<TestArray> {
    fn create() -> Seqloq<TestArray> {
        Seqloq::new(TestArray::new())
    }

    fn check(&self, delay: Duration) -> usize {
        let x = self.read();
        x.check(delay)
    }

    fn frob(&self, delay: Duration) {
        self.lock().unwrap().frob(delay);
    }
}

pub struct SeqloqPeek<T>(Seqloq<T>);

impl TestableMutex for SeqloqPeek<TestArray> {
    fn create() -> SeqloqPeek<TestArray> {
        SeqloqPeek(Seqloq::new(TestArray::new()))
    }

    fn check(&self, delay: Duration) -> usize {
        self.0.peek(|x| unsafe {
            (*x.unwrap()).check(delay)
        })
    }

    fn frob(&self, delay: Duration) {
        self.0.frob(delay);
    }
}

struct BogusMutex<T>(UnsafeCell<T>);

unsafe impl<T: 'static> Send for BogusMutex<T> { }
unsafe impl<T: 'static> Sync for BogusMutex<T> { }

impl TestableMutex for BogusMutex<TestArray> {
    fn create() -> BogusMutex<TestArray> {
        BogusMutex(UnsafeCell::new(TestArray::new()))
    }

    fn check(&self, delay: Duration) -> usize {
        unsafe {
            (*self.0.get()).check(delay)
        }
    }

    fn frob(&self, delay: Duration) {
        unsafe {
            (*self.0.get()).frob(delay);
        }
    }
}

#[derive(Copy)]
pub struct ThreadSpec {
    /// Number of threads to spawn.
    pub qty: u64,
    /// Number of times to operate in each thread.
    pub steps: u64,
    /// Delay on each step of the checking or incrementing operation, in microseconds
    pub delay: u64,
    /// Pause between operations, with the mutex unlocked, in microseconds
    pub pause: u64,
}

impl Default for ThreadSpec {
    fn default() -> ThreadSpec {
        ThreadSpec {
            qty: 100,
            steps: 100,
            delay: 2,
            pause: 2000,
        }
    }
}

impl ThreadSpec {
    fn pause(&self) {
        timer::sleep(Duration::microseconds(self.pause as i64));
    }
}

struct SharedData<M> {
    mutex: M,
    shutdown: AtomicBool,
    failed_checks: AtomicUsize,
}

#[derive(Copy, Show)]
pub enum BenchMode { Reader, Writer }

pub struct BenchRequest<'a> {
    pub mode: BenchMode,
    pub num_samples: u64,
    pub samples: &'a mut Vec<u64>,
}

pub fn reader_writer_test<M: TestableMutex>(
    readers: ThreadSpec,
    writers: ThreadSpec,
    bench: Option<BenchRequest>,
    should_fail: bool)
{
    // should be safe to put this on the stack, but screw it
    let shared = Arc::new(SharedData {
        mutex: <M as TestableMutex>::create(),
        shutdown: AtomicBool::new(false),
        failed_checks: AtomicUsize::new(0),
    });
    let mut guards = vec![];

    macro_rules! go {
        ($spec:ident, $is_writer:expr) => {
            for _ in 0..$spec.qty {
                let shared = shared.clone();
                guards.push(Thread::scoped(move || {
                    for _ in 0..$spec.steps {
                        let delay = Duration::microseconds($spec.delay as i64);
                        if $is_writer {
                            shared.mutex.frob(delay);
                        } else {
                            if 0 != shared.mutex.check(delay) {
                                shared.failed_checks.fetch_add(1, Ordering::SeqCst);
                            }
                        }

                        $spec.pause();
                        if shared.shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }));
            }
        }
    }

    go!(readers, false);
    go!(writers, true);

    if let Some(bench) = bench {
        for _ in 0..bench.num_samples {
            let t0;
            let t1;
            match bench.mode {
                BenchMode::Reader => {
                    t0 = precise_time_ns();
                    let res = shared.mutex.check(Duration::zero());
                    t1 = precise_time_ns();
                    assert_eq!(res, 0);
                    readers.pause();
                },

                BenchMode::Writer => {
                    t0 = precise_time_ns();
                    shared.mutex.frob(Duration::zero());
                    t1 = precise_time_ns();
                    writers.pause();
                },
            }
            bench.samples.push(t1 - t0);
        }

        shared.shutdown.store(true, Ordering::SeqCst);
    }

    for r in guards.into_iter() {
        if let Err(e) = r.join() {
            // can't print Box<Any + Send> :(
            panic!("thread panicked with {:?}", e.get_type_id());
        }
    }

    let failures = shared.failed_checks.load(Ordering::SeqCst);
    if should_fail {
        assert!(failures > 0);
    } else {
        assert_eq!(failures, 0);
    }
}

macro_rules! mk_test {
    ($name:ident, $mutex:ident) => {
        #[test]
        fn $name() {
            let spec = Default::default();
            reader_writer_test::<$mutex<TestArray>>(spec, spec, None, false);
        }
    }
}

mk_test!(test_mutex, Mutex);
mk_test!(test_rwlock, RwLock);
mk_test!(test_seqloq, Seqloq);
mk_test!(test_seqloq_peek, SeqloqPeek);

#[test]
fn test_bogus_mutex() {
    let spec = Default::default();
    reader_writer_test::<BogusMutex<TestArray>>(spec, spec, None, true);
}
