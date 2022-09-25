use std::{arch::asm, cell::Cell, ptr};

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
    pub cpu_id_start: u32,
    pub cpu_id: u32,
    pub rseq_cs: u64,
    pub flags: u32,
}

thread_local! {
    static RSEQ: Cell<*mut rseq> = const { Cell::new(ptr::null_mut()) };
}

#[inline(never)]
#[cold]
fn get_rseq_slow() -> *mut rseq {
    extern "C" {
        static __rseq_offset: usize;
    }
    let tp: *mut u8;
    let rseq: *mut rseq;
    unsafe {
        asm!("movq %fs:0, {tp}", tp = out(reg) tp, options(att_syntax));
        rseq = tp.add(__rseq_offset) as *mut rseq;
    }
    RSEQ.with(|thread_local| thread_local.set(rseq));
    rseq
}

#[inline(always)]
pub fn get_rseq() -> *mut rseq {
    let rseq = RSEQ.with(|thread_local| thread_local.get());
    if rseq.is_null() {
        return get_rseq_slow();
    }
    rseq
}

// #[inline(always)]
// pub fn get_rseq() -> *mut rseq {
//     extern "C" {
//         static __rseq_offset: usize;
//     }
//     unsafe {
//         let fs: *mut u8;
//         asm!(
//         "movq %fs:0, {fs}",
//         fs = out(reg) fs,
//         options(att_syntax),
//         );
//         fs.add(__rseq_offset) as _
//     }
// }
