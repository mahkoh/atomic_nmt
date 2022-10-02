#![allow(unused_doc_comments)]

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[path = "x86_64.rs"]
        pub(crate) mod arch;
    }
}

pub use arch::acquire;
use {
    crate::{
        nmt::inner::{
            cache_line::CacheLineAligned,
            per_cpu_thread::run_on_cpu,
            rseq::{get_rseq, rseq},
        },
        stats::NUM_OFF_CPU_RELEASE,
    },
    cfg_if::cfg_if,
    std::sync::atomic::Ordering::Relaxed,
};

/// A reference to a value that is owned by a single CPU.
#[repr(C)]
pub struct PerCpuRc<T> {
    /// The reference count. If this drops to 0, the object will be freed.
    rc: u64,
    /// The id of the CPU that owns this structure. Not modified after initialization.
    cpu_id: u32,
    /// The stored value. Not modified after initialization.
    pub value: T,
    /// Ensure that this structure is cache-line aligned.
    _aligned: CacheLineAligned<()>,
}

/// Allocates a new per-cpu value for the given cpu.
pub fn new<T: Send + Sync>(cpu_id: u32, value: T) -> *mut PerCpuRc<T> {
    Box::leak(Box::new(PerCpuRc {
        rc: 1,
        cpu_id,
        value,
        _aligned: Default::default(),
    }))
}

// The following constants are the return values of `lazy_atomic_release_thread_pointer`.

/// The reference count was reduced by 1 and is now > 0.
const ALIVE: u64 = 0;
/// The reference count was reduced by 1 and is now = 0. The object should be deallocated.
const DEAD: u64 = 1;
/// The reference count was not reduced because the function ran on a CPU that does not own
/// the per-cpu data. The operation should be retried.
#[allow(dead_code)]
const OFF_CPU: u64 = 2;

//////////////////////////////////////
// RELEASE
//////////////////////////////////////

/// Releases a reference to a `PerCpuRc`.
///
/// # Safety
///
/// This function takes ownership of the reference. After this function returns, the
/// reference must no longer be accessed.
///
/// The reference must be a pointer returned from `new` above.
#[inline]
pub unsafe fn release<T: Send + Sync>(rseq: *mut rseq, data: &PerCpuRc<T>) {
    let cpu_id = data.cpu_id;
    let data = data as *const _ as _;
    let res = arch::release(rseq, data);
    if res != ALIVE {
        // We have to either deallocate the per-cpu data or retry the release.
        release_slow::<T>(res, cpu_id, data);
    }
}

#[cold]
unsafe fn release_slow<T: Send + Sync>(res: u64, cpu_id: u32, data: *mut PerCpuRc<T>) {
    if res == DEAD {
        // The reference count has been reduced to 0. Deallocate the data.
        drop(Box::from_raw(data));
        return;
    }
    // res == OFF_CPU. This is the very-very slow path. Send the reference to the per-cpu thread
    // so that it can be unreferenced there.
    release_off_cpu(cpu_id, data);
}

#[inline(never)]
unsafe fn release_off_cpu<T: Send + Sync>(cpu_id: u32, data: *mut PerCpuRc<T>) {
    NUM_OFF_CPU_RELEASE.fetch_add(1, Relaxed);
    // We have to cast the pointer to usize because pointers are not `Send`.
    let data = data as usize;
    run_on_cpu(
        cpu_id as usize,
        Box::new(move || {
            // Restore the pointer.
            let data = data as *mut PerCpuRc<T>;
            // NOTE: This code runs only on the cpu that owns the data. However, we cannot simply
            // reduce the reference count using non-atomic operations. If we were to be rescheduled
            // after checking the current reference count but before decrementing it, the behavior
            // would be undefined. We could use atomic operations but that would be slower than
            // using rseq.
            let rseq = get_rseq();
            let res = arch::release(rseq, data);
            if res == DEAD {
                drop(Box::from_raw(data));
            } else {
                // Sanity check.
                assert_eq!(res, ALIVE);
            }
        }),
    );
}
