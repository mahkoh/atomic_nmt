//! This crate provides an [eventually-consistent][ec] generic atomic type for arbitrary
//! `Clone + Send + Sync + 'static` values.
//!
//! [ec]: https://en.wikipedia.org/wiki/Eventual_consistency

pub use {
    nmt::{inner::per_cpu_thread::run_on_cpu, AtomicNmt},
    slc::AtomicSlc,
};

mod nmt;
mod slc;

/// Statistics
pub mod stats {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    pub(super) static NUM_OFF_CPU_RELEASE: AtomicUsize = AtomicUsize::new(0);

    /// How often objects had to sent to a per-cpu thread to be released due to thread migration.
    ///
    /// This should usually happen in <0.1% of `get` calls.
    pub fn num_off_cpu_release() -> u64 {
        NUM_OFF_CPU_RELEASE.load(Relaxed) as _
    }
}

pub fn set_priority(p: i32) {
    // return;
    unsafe {
        let param = libc::sched_param {
            sched_priority: p as _,
        };
        let res = libc::sched_setscheduler(0, libc::SCHED_RR, &param);
        assert_eq!(res, 0);
    }
}
