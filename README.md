# seqloq

[Seqlocks][] for Rust, inspired by the [Linux kernel's implementation][].

seqloq supports optimistic, lock-free reads of thread-shared data. The reader
checks a sequence number before and after reading, and retries if the data
structure changed during the read.  When the workload consists mainly of reads,
this can improve performance vs. a traditional mutex.

Here's a histogram for read latency on a 32-byte data structure, with a few
infrequent writes and 200 concurrent readers:

![read histogram](https://raw.githubusercontent.com/kmcallister/seqloq/master/doc/histogram.read.png)

The write performance is also competitive:

![write histogram](https://raw.githubusercontent.com/kmcallister/seqloq/master/doc/histogram.write.png)

seqloq can also prevent starvation of writers, because writers have absolute
priority.

To render your own histograms:

```
cargo build --release
./target/release/bench
./plot.py
```

The hard-coded ranges in `plot.py` will probably need adjustment for your
machine.

[Seqlocks]: http://en.wikipedia.org/wiki/Seqlock
[Linux kernel's implementation]: https://github.com/torvalds/linux/blob/master/include/linux/seqlock.h
