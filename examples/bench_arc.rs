#![feature(bench_black_box)]

use {
    crossbeam::atomic::AtomicCell,
    lazy_atomic::{stats::num_off_cpu_release, AtomicNmt, AtomicSlc},
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
    // atomic_nmt();
    atomic_slc();
    // mutex();
    // rwlock();
}

macro_rules! value {
    () => {
        // "hello world".to_owned()
        // SystemTime::now()
        Arc::new(1)
        // Arc::new(SystemTime::now())
        // Arc::new("hello world".to_owned())
        // [1u64; 128]
    };
}

const ITERATIONS: usize = 1_000_000;

const NUM_READERS: usize = 4;
const NUM_WRITERS: usize = 4;
const NOPS_AFTER_WRITE: usize = 1_000;

fn nops() {
    thread::sleep(Duration::from_millis(10));
    // thread::yield_now();
    for _ in 0..NOPS_AFTER_WRITE {
        unsafe {
            asm!("");
        }
    }
}

fn atomic_nmt() {
    let atomic = AtomicNmt::new(value!());
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
    for _ in 0..ITERATIONS {
        black_box(atomic.get());
    }
    let now = Instant::now();
    for _ in 0..ITERATIONS {
        // println!("{:?}", atomic.get());
        black_box(atomic.get());
    }
    let elapsed = now.elapsed();
    println!("atomic: {:?}", elapsed);
    println!("off-cpu releases: {}", num_off_cpu_release());
}

fn atomic_slc() {
    let mut atomic = AtomicSlc::new(value!());
    for _ in 0..NUM_READERS {
        let mut atomic = atomic.clone();
        thread::spawn(move || loop {
            atomic.get();
        });
    }
    for _ in 0..NUM_WRITERS {
        let mut atomic = atomic.clone();
        thread::spawn(move || loop {
            atomic.set(value!());
            nops();
        });
    }
    thread::sleep(Duration::from_secs(1));
    for _ in 0..ITERATIONS {
        black_box(atomic.get());
    }
    let now = Instant::now();
    for _ in 0..ITERATIONS {
        // println!("{:?}", atomic.get());
        black_box(atomic.get());
    }
    let elapsed = now.elapsed();
    println!("atomic: {:?}", elapsed);
    println!("off-cpu releases: {}", num_off_cpu_release());
}

// fn crossbeam() {
//     let atomic = Arc::new(AtomicCell::new(value!()));
//     for _ in 0..NUM_READERS {
//         let atomic = atomic.clone();
//         thread::spawn(move || loop {
//             black_box(atomic.load());
//         });
//     }
//     for _ in 0..NUM_WRITERS {
//         let atomic = atomic.clone();
//         thread::spawn(move || loop {
//             black_box(atomic.store(value!()));
//             nops();
//         });
//     }
//     thread::sleep(Duration::from_secs(1));
//     for _ in 0..ITERATIONS {
//         black_box(atomic.load());
//     }
//     let now = Instant::now();
//     for _ in 0..ITERATIONS {
//         black_box(atomic.load());
//     }
//     let elapsed = now.elapsed();
//     println!("crossbeam: {:?}", elapsed);
// }

fn mutex() {
    let atomic = Arc::new(Mutex::new(value!()));
    for _ in 0..NUM_READERS {
        let atomic = atomic.clone();
        thread::spawn(move || loop {
            black_box(atomic.lock().deref());
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
    for _ in 0..ITERATIONS {
        black_box(atomic.lock().deref());
    }
    let now = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(atomic.lock().deref());
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
    for _ in 0..ITERATIONS {
        black_box(atomic.read().clone());
    }
    let now = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(atomic.read().clone());
    }
    let elapsed = now.elapsed();
    println!("lock: {:?}", elapsed);
}
