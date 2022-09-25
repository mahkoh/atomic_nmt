#![allow(unused_doc_comments)]

#[cfg(target_arch = "x86_64")]
mod x86_64;

use {
    crate::rseq::{
        per_cpu_thread::run_on_cpu,
        rseq::{get_rseq, rseq},
    },
    std::{cell::Cell, mem},
};

#[repr(C, align(64))]
pub struct PerCpuRc<T> {
    pub rc: u64,
    pub cpu_id: u32,
    pub value: T,
}

pub fn new<T: Send + Sync>(cpu_id: u32, value: T) -> *mut PerCpuRc<T> {
    Box::leak(Box::new(PerCpuRc {
        rc: 1,
        cpu_id,
        value,
    }))
}

const ALIVE: u64 = 0;
const DEAD: u64 = 1;
#[allow(dead_code)]
const OFF_CPU: u64 = 2;

extern "C" {
    fn lazy_transform_acquire_thread_pointer(
        rseq: *mut rseq,
        data_by_cpu: &[Cell<*mut PerCpuRc<u8>>],
    ) -> (u32, *mut PerCpuRc<u8>);

    fn lazy_transform_release_thread_pointer(
        rseq: *mut rseq,
        data_by_cpu: *const PerCpuRc<u8>,
    ) -> u64;
}

//////////////////////////////////////
// ACQUIRE
//////////////////////////////////////

#[inline]
pub unsafe fn acquire<T: Send + Sync>(
    data_by_cpu: &[Cell<*mut PerCpuRc<T>>],
) -> (usize, Option<&PerCpuRc<T>>) {
    let rseq = get_rseq();
    let data_by_cpu = mem::transmute(data_by_cpu);
    let (cpu_id, data) = lazy_transform_acquire_thread_pointer(rseq, data_by_cpu);
    (cpu_id as _, mem::transmute(data))
}

//////////////////////////////////////
// RELEASE
//////////////////////////////////////

#[inline]
pub unsafe fn release<T: Send + Sync>(data: &PerCpuRc<T>) {
    let cpu_id = data.cpu_id;
    let data = data as *const _ as *mut PerCpuRc<T>;
    let rseq = get_rseq();
    let res = lazy_transform_release_thread_pointer(rseq, data as _);
    if res != ALIVE {
        release_slow(res, cpu_id, data);
    }
}

#[cold]
unsafe fn release_slow<T: Send + Sync>(res: u64, cpu_id: u32, data: *mut PerCpuRc<T>) {
    if res == DEAD {
        drop(Box::from_raw(data));
        return;
    }
    release_off_cpu(cpu_id, data);
}

#[inline(never)]
unsafe fn release_off_cpu<T: Send + Sync>(cpu_id: u32, data: *mut PerCpuRc<T>) {
    // println!("off cpu");
    let data = data as usize;
    run_on_cpu(
        cpu_id as usize,
        Box::new(move || {
            let data = data as *mut PerCpuRc<T>;
            let rseq = get_rseq();
            let res = lazy_transform_release_thread_pointer(rseq, data as _);
            if res == DEAD {
                drop(Box::from_raw(data));
            }
        }),
    );
}
