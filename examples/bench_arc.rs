#![feature(bench_black_box)]

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::atomic::Ordering::Relaxed;
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
    // mutex_slc();
    // mutex();
    // rwlock();
}

macro_rules! value {
    () => {
        // "hello world".to_owned()
        // SystemTime::now()
        // Arc::new(1)
        // Arc::new(SystemTime::now())
        // Arc::new("hello world".to_owned())
        [1u64; 1]
    };
}

const ITERATIONS: usize = 1_000_000;

const NUM_READERS: usize = 0;
const NUM_WRITERS: usize = 7;
const NOPS_AFTER_WRITE: usize = 0;

fn nops() {
    // thread::sleep(Duration::from_micros(10));
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
            set_priority(1);
            atomic.set(value!());
            nops();
        });
    }
    set_priority(99);
    thread::sleep(Duration::from_secs(1));
    // for _ in 0..ITERATIONS {
    //     black_box(atomic.get());
    // }
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

fn mutex_slc() {
    struct Inner<T> {
        version: AtomicU64,
        value: Mutex<T>,
    }

    struct Outer<T> {
        value: T,
        version: u64,
        inner: Arc<Inner<T>>,
    }

    impl<T: Clone> Outer<T> {
        #[cold]
        fn update(&mut self) {
            let inner = self.inner.value.lock();
            self.value = (*inner).clone();
            self.version = self.inner.version.load(Relaxed);
        }

        fn get(&mut self) -> &T {
            if self.version < self.inner.version.load(Relaxed) {
                self.update();
            }
            &self.value
        }
    }

    let inner = Arc::new(Inner {
        version: AtomicU64::new(0),
        value: Mutex::new(value!()),
    });
    for _ in 0..NUM_READERS {
        let mut outer = Outer {
            value: value!(),
            version: 0,
            inner: inner.clone(),
        };
        thread::spawn(move || loop {
            black_box(outer.get());
        });
    }
    for _ in 0..NUM_WRITERS {
        let inner = inner.clone();
        thread::spawn(move || loop {
            set_priority(1);
            let _old;
            {
                let mut lock = inner.value.lock();
                _old = mem::replace(lock.deref_mut(), value!());
                inner.version.store(inner.version.load(Relaxed) + 1, Relaxed);
            }
            nops();
        });
    }
    let mut outer = Outer {
        value: value!(),
        version: 0,
        inner: inner.clone(),
    };
    set_priority(99);
    thread::sleep(Duration::from_secs(1));
    for _ in 0..ITERATIONS {
        black_box(outer.get());
    }
    let now = Instant::now();
    for _ in 0..ITERATIONS {
        // println!("{:?}", outer.get());
        black_box(outer.get());
    }
    let elapsed = now.elapsed();
    println!("mutex slc: {:?}", elapsed);
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

fn set_priority(p: i32) {
    unsafe {
        let param = libc::sched_param {
            sched_priority: p as _,
        };
        let res = libc::sched_setscheduler(0, libc::SCHED_FIFO, &param);
        assert_eq!(res, 0);
    }
}
