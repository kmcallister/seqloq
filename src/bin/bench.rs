#![deny(warnings)]
#![allow(unstable)]

extern crate seqloq;

use std::sync::{Mutex, RwLock};
use std::default::Default;
use std::io::File;
use std::path::Path;

use seqloq::Seqloq;
use seqloq::tests::{TestArray, BenchMode, BenchRequest, ThreadSpec};
use seqloq::tests::{SeqloqPeek, reader_writer_test};

pub fn main() {
    // Infrequent writes
    let writers = ThreadSpec {
        qty: 3,
        pause: 2000,
        ..Default::default()
    };
    // Demanding readers
    let readers = ThreadSpec {
        qty: 200,
        pause: 0,
        ..Default::default()
    };

    macro_rules! bench_one {
        ($name:expr, $mutex:ident, $mode:ident) => ({
            let mut samples = vec![];

            {
                let bench = BenchRequest {
                    mode: BenchMode::$mode,
                    num_samples: 10_000,
                    samples: &mut samples,
                };
                reader_writer_test::<$mutex<TestArray>>(readers, writers,
                    Some(bench), false);
            }

            let mut out = File::create(&Path::new(concat!("target/", $name))).unwrap();
            for sample in samples.iter() {
                writeln!(&mut out, "{}", sample)
                    .unwrap();
            }
        })
    }

    bench_one!("mutex_read.dat", Mutex,  Reader);
    bench_one!("rwlock_read.dat", RwLock, Reader);
    bench_one!("seqloq_read.dat", Seqloq, Reader);
    bench_one!("seqloq-peek_read.dat", SeqloqPeek, Reader);

    bench_one!("mutex_write.dat", Mutex,  Writer);
    bench_one!("rwlock_write.dat", RwLock, Writer);
    bench_one!("seqloq_write.dat", Seqloq, Writer);
    bench_one!("seqloq-peek_write.dat", SeqloqPeek, Writer);
}
