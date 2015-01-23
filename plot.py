#!/usr/bin/env python
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

def read_data(fn):
    return [int(x) for x in file('target/'+fn+'.dat').readlines()]

def plot(which, bins):
    plt.cla()
    plt.hist(read_data('mutex_'+which),  label="Mutex",  bins=bins, color='g', alpha=0.4)
    plt.hist(read_data('rwlock_'+which), label="RwLock", bins=bins, color='b', alpha=0.4)
    plt.hist(read_data('seqloq_'+which), label="Seqloq", bins=bins, color='r', alpha=0.4)
    plt.xlabel(which.title() + " time (nanoseconds)")
    plt.ylabel("Count out of 10,000")
    plt.legend()
    plt.savefig('histogram.'+which+'.png')

plot('read', range(150, 200, 2))
plot('write', range(600, 3750, 40))
