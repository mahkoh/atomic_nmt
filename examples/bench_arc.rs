#![feature(bench_black_box)]

use {
    crossbeam::atomic::AtomicCell,
    lazy_atomic::{stats::num_off_cpu_release, Atomic},
    parking_lot::{Mutex, RwLock},
    std::{
        arch::asm,
        hint::black_box,
        mem,
        ops::{Deref, DerefMut},
        sync::Arc,
        thread,
        time::{Duration, Instant, SystemTime},
    },
};

fn main() {
    // crossbeam();
    // mutex();
    // rwlock();
    atomic();
}

macro_rules! value {
    () => {
        SystemTime::now()
        // [1u64; 2]
    };
}

const NUM_READERS: usize = 0;
const NUM_WRITERS: usize = 1;
const NOPS_AFTER_WRITE: usize = 0;

fn nops() {
    for _ in 0..NOPS_AFTER_WRITE {
        unsafe {
            asm!("");
        }
    }
}

fn atomic() {
    let atomic = Atomic::new(value!());
    for _ in 0..NUM_READERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            atomic.get();
        });
    }
    for _ in 0..NUM_WRITERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            atomic.set(value!());
            nops();
        });
    }
    thread::sleep(Duration::from_secs(1));
    for _ in 0..1_000_000 {
        black_box(atomic.get());
    }
    let now = Instant::now();
    for _ in 0..1_000_000 {
        println!("{:?}", atomic.get());
        // black_box(atomic.get());
    }
    let elapsed = now.elapsed();
    println!("atomic: {:?}", elapsed);
    println!("off-cpu releases: {}", num_off_cpu_release());
}

fn crossbeam() {
    let atomic = Arc::new(AtomicCell::new(value!()));
    for _ in 0..NUM_READERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            black_box(atomic.load());
        });
    }
    for _ in 0..NUM_WRITERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            black_box(atomic.store(value!()));
            nops();
        });
    }
    thread::sleep(Duration::from_secs(1));
    for _ in 0..1_000_000 {
        black_box(atomic.load());
    }
    let now = Instant::now();
    for _ in 0..1_000_000 {
        black_box(atomic.load());
    }
    let elapsed = now.elapsed();
    println!("crossbeam: {:?}", elapsed);
}

fn mutex() {
    let atomic = Arc::new(Mutex::new(value!()));
    for _ in 0..NUM_READERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            black_box(atomic.lock().clone());
        });
    }
    for _ in 0..NUM_WRITERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            let _v = mem::replace(atomic.lock().deref_mut(), value!());
            nops();
        });
    }
    thread::sleep(Duration::from_secs(1));
    for _ in 0..1_000_000 {
        black_box(atomic.lock().deref().clone());
    }
    let now = Instant::now();
    for _ in 0..1_000_000 {
        black_box(atomic.lock().deref().clone());
    }
    let elapsed = now.elapsed();
    println!("lock: {:?}", elapsed);
}

fn rwlock() {
    let atomic = Arc::new(RwLock::new(value!()));
    for _ in 0..NUM_READERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            black_box(atomic.read().clone());
        });
    }
    for _ in 0..NUM_WRITERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            let _v = mem::replace(atomic.write().deref_mut(), value!());
            nops();
        });
    }
    thread::sleep(Duration::from_secs(1));
    for _ in 0..1_000_000 {
        black_box(atomic.read().clone());
    }
    let now = Instant::now();
    for _ in 0..1_000_000 {
        black_box(atomic.read().clone());
    }
    let elapsed = now.elapsed();
    println!("lock: {:?}", elapsed);
}
