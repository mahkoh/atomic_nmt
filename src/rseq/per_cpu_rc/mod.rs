#![allow(unused_doc_comments)]

#[cfg(target_arch = "x86_64")]
mod x86_64;

use {
    crate::{
        rseq::{
            cache_line::CacheLineAligned,
            per_cpu_thread::run_on_cpu,
            rseq::{get_rseq, rseq},
        },
        stats::NUM_OFF_CPU_RELEASE,
    },
    std::{cell::Cell, mem, sync::atomic::Ordering::Relaxed},
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

// Extern functions written in platform-specific assembly.
extern "C" {
    /// This function is essentially
    ///
    /// ```no_run
    /// use std::cell::Cell;
    /// unsafe extern fn lazy_atomic_acquire_thread_pointer(
    ///     rseq: *mut rseq,
    ///     data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<u8>>>],
    /// ) -> (u32, *mut PerCpuRc<u8>) {
    ///     let cpu = (*rseq).cpu_id;
    ///     let data = *data_by_cpu.get_unchecked(cpu as usize).0.get();
    ///     if !data.is_null() {
    ///         (*data).rc += 1;
    ///     }
    ///     (cpu, data)
    /// }
    /// ```
    ///
    /// However, it guarantees that the entire function body is executed without interruption, that
    /// is, without the thread being moved to a different cpu, without a signal being caught, without
    /// the thread being rescheduled, etc.
    ///
    /// Returns the id of the cpu it ran on and an owned reference to the per-cpu data of that cpu.
    /// The reference is null if the corresponding pointer in the slice was null.
    fn lazy_atomic_acquire_thread_pointer(
        rseq: *mut rseq,
        data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<u8>>>],
    ) -> (u32, *mut PerCpuRc<u8>);

    /// This function is essentially the following but with the same guarantees as above.
    ///
    /// ```
    /// unsafe extern fn lazy_atomic_release_thread_pointer(
    ///     rseq: *mut rseq,
    ///     data: *mut PerCpuRc<u8>,
    /// ) -> u64 {
    ///     let cpu = (*rseq).cpu_id;
    ///     let mut res = OFF_CPU;
    ///     if cpu == (*data).cpu_id {
    ///         res = ALIVE;
    ///         if (*data).rc == 1 {
    ///             res = DEAD;
    ///         }
    ///         (*data).rc -= 1;
    ///     }
    ///     res
    /// }
    /// ```
    ///
    /// Returns one of `OFF_CPU`, `ALIVE`, `DEAD`. The meaning of these return values is documented
    /// above.
    fn lazy_atomic_release_thread_pointer(rseq: *mut rseq, data_by_cpu: *const PerCpuRc<u8>)
        -> u64;
}

//////////////////////////////////////
// ACQUIRE
//////////////////////////////////////

/// Acquires a reference to per-cpu data.
///
/// # Safety
///
/// The length of the slice must be larger than the largest possible cpu id.
///
/// The pointers stored in the slice must either be null or valid pointers that were
/// previously returned by `new`.
#[inline]
pub unsafe fn acquire<T: Send + Sync>(
    data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<T>>>],
) -> (usize, Option<&PerCpuRc<T>>) {
    let rseq = get_rseq();
    // SAFETY: T and u8 are Sized. Therefore their pointers have compatible representations.
    let data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<u8>>>] = mem::transmute(data_by_cpu);
    let (cpu_id, data) = lazy_atomic_acquire_thread_pointer(rseq, data_by_cpu);
    // SAFETY: data is one of the pointers stored in data_by_cpu.
    let data: Option<&PerCpuRc<T>> = mem::transmute(data);
    (cpu_id as _, data)
}

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
pub unsafe fn release<T: Send + Sync>(data: &PerCpuRc<T>) {
    let cpu_id = data.cpu_id;
    let data = data as *const _ as *mut PerCpuRc<T>;
    let rseq = get_rseq();
    let res = lazy_atomic_release_thread_pointer(rseq, data as _);
    if res != ALIVE {
        // We have to either deallocate the per-cpu data or retry the release.
        release_slow(res, cpu_id, data);
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
            let res = lazy_atomic_release_thread_pointer(rseq, data as _);
            if res == DEAD {
                drop(Box::from_raw(data));
            } else {
                // Sanity check.
                assert_eq!(res, ALIVE);
            }
        }),
    );
}
