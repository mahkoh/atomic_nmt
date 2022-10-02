//! From `linux/kernel/rseq.c`:
//!
//! /*
//!  * Restartable sequences are a lightweight interface that allows
//!  * user-level code to be executed atomically relative to scheduler
//!  * preemption and signal delivery. Typically used for implementing
//!  * per-cpu operations.
//!  *
//!  * It allows user-space to perform update operations on per-cpu data
//!  * without requiring heavy-weight atomic operations.
//!  *
//!  * Detailed algorithm of rseq user-space assembly sequences:
//!  *
//!  *                     init(rseq_cs)
//!  *                     cpu = TLS->rseq::cpu_id_start
//!  *   [1]               TLS->rseq::rseq_cs = rseq_cs
//!  *   [start_ip]        ----------------------------
//!  *   [2]               if (cpu != TLS->rseq::cpu_id)
//!  *                             goto abort_ip;
//!  *   [3]               <last_instruction_in_cs>
//!  *   [post_commit_ip]  ----------------------------
//!  *
//!  *   The address of jump target abort_ip must be outside the critical
//!  *   region, i.e.:
//!  *
//!  *     [abort_ip] < [start_ip]  || [abort_ip] >= [post_commit_ip]
//!  *
//!  *   Steps [2]-[3] (inclusive) need to be a sequence of instructions in
//!  *   userspace that can handle being interrupted between any of those
//!  *   instructions, and then resumed to the abort_ip.
//!  *
//!  *   1.  Userspace stores the address of the struct rseq_cs assembly
//!  *       block descriptor into the rseq_cs field of the registered
//!  *       struct rseq TLS area. This update is performed through a single
//!  *       store within the inline assembly instruction sequence.
//!  *       [start_ip]
//!  *
//!  *   2.  Userspace tests to check whether the current cpu_id field match
//!  *       the cpu number loaded before start_ip, branching to abort_ip
//!  *       in case of a mismatch.
//!  *
//!  *       If the sequence is preempted or interrupted by a signal
//!  *       at or after start_ip and before post_commit_ip, then the kernel
//!  *       clears TLS->__rseq_abi::rseq_cs, and sets the user-space return
//!  *       ip to abort_ip before returning to user-space, so the preempted
//!  *       execution resumes at abort_ip.
//!  *
//!  *   3.  Userspace critical section final instruction before
//!  *       post_commit_ip is the commit. The critical section is
//!  *       self-terminating.
//!  *       [post_commit_ip]
//!  *
//!  *   4.  <success>
//!  *
//!  *   On failure at [2], or if interrupted by preempt or signal delivery
//!  *   between [1] and [3]:
//!  *
//!  *       [abort_ip]
//!  *   F1. <failure>
//!  */

use std::{arch::asm, cell::Cell, ptr};

/// This struct is here merely for illustration. Actual instances of the struct are defined
/// in assembly.
#[allow(dead_code)]
#[repr(C, align(32))]
pub struct rseq_cs {
    pub version: u32,
    pub flags: u32,
    pub start_ip: u64,
    pub post_commit_offset: u64,
    pub abort_ip: u64,
}

#[repr(C, align(32))]
pub struct rseq {
    /// The following two variables have the same value for our purposes. In general,
    /// `cpu_id_start` differs from `cpu_id` in that it `cpu_id_start` always contains
    /// a valid cpu id whereas `cpu_id` can contain error values.
    ///
    /// We do not handle such errors. For our purposes, `cpu_id` always contains the id
    /// of the cpu we're currently executing on.
    pub cpu_id_start: u32,
    pub cpu_id: u32,
    /// Pointer to the currently active `resq_cs` cast to u64.
    pub rseq_cs: u64,
    pub flags: u32,
}

thread_local! {
    /// Contains a pointer to the thread's rseq structure or null if it's never been accessed.
    static RSEQ: Cell<*mut rseq> = const { Cell::new(ptr::null_mut()) };
}

#[inline(never)]
#[cold]
fn get_rseq_slow() -> *mut rseq {
    extern "C" {
        /// The offset of the rseq structure within the thread area. See
        /// glibc/sysdeps/unix/sysv/linux/sys/rseq.h
        static __rseq_offset: usize;
    }
    let rseq: *mut rseq;
    unsafe {
        let tp: *mut u8;
        #[cfg(target_arch = "x86_64")]
        {
            // NOTE: %fs:0 contains the address of the thread area. See
            // https://stackoverflow.com/questions/6611346/how-are-the-fs-gs-registers-used-in-linux-amd64/33827186#33827186
            asm!("movq %fs:0, {tp}", tp = out(reg) tp, options(att_syntax));
        }
        rseq = tp.add(__rseq_offset) as *mut rseq;
    }
    RSEQ.with(|thread_local| thread_local.set(rseq));
    rseq
}

/// Returns the current thread's rseq pointer.
#[inline(always)]
pub fn get_rseq() -> *mut rseq {
    let rseq = RSEQ.with(|thread_local| thread_local.get());
    if rseq.is_null() {
        // Initialize `RSEQ`.
        return get_rseq_slow();
    }
    rseq
}

#[inline(always)]
pub fn get_cpu() -> usize {
    unsafe {
        (*get_rseq()).cpu_id as _
    }
}

/// Checks if rseq support is available in the main thread of the process. Does not mean
/// that rseq support is available in other threads. For example, seccomp might have
/// prevented registration in non-main threads. We don't handle such situations.
pub fn ensure_enabled() {
    extern "C" {
        /// The size of the rseq struct. 0 if registration failed. See
        /// glibc/sysdeps/unix/sysv/linux/sys/rseq.h
        static __rseq_size: usize;
    }
    unsafe {
        assert!(
            __rseq_size > 0,
            "rseq is not available or has been disabled"
        );
    }
}

// NOTE: Despite not having a branch, the following code is slower than the above.
// #[inline(always)]
// pub fn get_rseq() -> *mut rseq {
//     extern "C" {
//         static __rseq_offset: usize;
//     }
//     unsafe {
//         let fs: *mut u8;
//         asm!(
//             "movq %fs:0, {fs}",
//             fs = out(reg) fs,
//             options(att_syntax),
//         );
//         fs.add(__rseq_offset) as _
//     }
// }
