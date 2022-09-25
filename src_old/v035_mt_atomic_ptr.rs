use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::{iter, ptr};
use std::arch::global_asm;
use std::cell::Cell;
use std::sync::Arc;
use crate::v035_mt_atomic_ptr::acquire_thread_pointer::acquire_thread_pointer;

#[test]
fn test() {
    let mut lt = LazyTransform::<u32, _, _>::new(|v| v);
    lt.set_source(Some(1));
    for _ in 0..1_000_000_00 {
        lt.get_value();
    }
}

pub struct LazyTransform<T, S, F> {
    transform_fn: F,
    source: AtomicPtr<S>,
    value: Mutex<Option<T>>,
    cpu_value: Vec<Cell<*mut SimpleArcInner<u32>>>,
}

impl<T, S, F> LazyTransform<T, S, F>
where
    T: Clone,
    F: Fn(S) -> Option<T>,
{
    pub fn new(transform_fn: F) -> Self {
        Self {
            transform_fn,
            source: Default::default(),
            value: Default::default(),
            cpu_value: iter::repeat_with(|| Cell::new(Box::into_raw(Box::new(SimpleArcInner { ref_count: Default::default(), data: 0 }))))
                .take(num_cpus::get())
                .collect(),
        }
    }

    pub fn set_source(&self, source: S) {
        self.source.store(Box::into_raw(Box::new(source)), Release);
    }

    #[inline]
    pub fn get_value(&self) -> Option<SimpleArc<u32>> {
        // if !self.source.load(Relaxed).is_null() {
        //     let source = self.source.swap(ptr::null_mut(), Acquire);
        //     if !source.is_null() {
        //         let source = unsafe { Box::from_raw(source) };
        //         *self.value.lock() = (self.transform_fn)(*source);
        //         for cpu in &self.cpu_needs_update {
        //             cpu.store(true, Release);
        //         }
        //     }
        // }
        acquire_thread_pointer(&self.cpu_value)
    }
}

#[repr(transparent)]
pub struct SimpleArc<T> {
    inner: *mut SimpleArcInner<T>,
}

impl<T> Drop for SimpleArc<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = &*self.inner;
            inner.ref_count.set(inner.ref_count.get() - 1);
        }
    }
}

#[repr(C)]
struct SimpleArcInner<T> {
    ref_count: Cell<u64>,
    data: T,
}

mod acquire_thread_pointer {
    use std::arch::global_asm;
    use std::cell::Cell;
    use std::mem;
    use std::sync::atomic::AtomicPtr;
    use crate::v035_mt_atomic_ptr::{SimpleArc, SimpleArcInner};
    use crate::v035_mt_atomic_ptr::rseq::{with_rseq};

    /// ```
    /// use std::cell::Cell;
    ///
    /// unsafe extern fn lazy_transform_acquire_thread_pointer(
    ///     data: &[Cell<*mut SimpleArcInner<u8>>],
    ///     cpu: &u32
    /// ) -> *mut SimpleArcInner<u8> {
    ///     let cpu = *cpu as usize;
    ///     let inner = data[cpu].get();
    ///     if !inner.is_null() {
    ///         let inner = &*inner;
    ///         inner.ref_count += 1;
    ///     }
    ///     inner
    /// }
    /// ```
    // language=asm
    global_asm!(r#"

    .global lazy_transform_acquire_thread_pointer
	.section .text.lazy_transform_acquire_thread_pointer,"x",@progbits
    .type lazy_transform_acquire_thread_pointer,@function
	.align 32
lazy_transform_acquire_thread_pointer:
    movl (%rdx), %eax
    movq (%rdi,%rax,8), %rax
    # testq  %rax, %rax
    # je lazy_transform_acquire_thread_pointer_post_commit_ip
    # incq (%rax)
lazy_transform_acquire_thread_pointer_post_commit_ip:
    ret
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
lazy_transform_acquire_thread_pointer_abort_ip:
    jmp lazy_transform_acquire_thread_pointer
    .size lazy_transform_acquire_thread_pointer, . - lazy_transform_acquire_thread_pointer



    .global lazy_transform_acquire_thread_pointer_rseq_cs
	.section .rodata.lazy_transform_acquire_thread_pointer_rseq_cs,"a",@progbits
	.align 32
lazy_transform_acquire_thread_pointer_rseq_cs:
    .long 0
    .long 0
    .quad lazy_transform_acquire_thread_pointer
    .quad lazy_transform_acquire_thread_pointer_post_commit_ip - lazy_transform_acquire_thread_pointer
    .quad lazy_transform_acquire_thread_pointer_abort_ip

"#, options(att_syntax));

    extern {
        #[allow(improper_ctypes)]
        fn lazy_transform_acquire_thread_pointer(data: &[Cell<*mut SimpleArcInner<u8>>], cpu: &u32) -> *mut SimpleArcInner<u8>;

        static lazy_transform_acquire_thread_pointer_rseq_cs: u8;
    }


    pub(super) fn acquire_thread_pointer<T>(headers: &[Cell<*mut SimpleArcInner<T>>]) -> Option<SimpleArc<T>> {
        unsafe {
            with_rseq(&lazy_transform_acquire_thread_pointer_rseq_cs, |cpu| {
                let headers = mem::transmute(headers);
                let arc = lazy_transform_acquire_thread_pointer(headers, cpu);
                if arc.is_null() {
                    None
                } else {
                    Some(SimpleArc {
                        inner: mem::transmute(arc),
                    })
                }
            })
        }
    }
}

mod rseq {
    use std::arch::asm;
    use std::cell::Cell;
    use std::ptr;
    use std::sync::atomic::AtomicU32;

    #[repr(C, align(32))]
    struct rseq {
        cpu_id_start: u32,
        cpu_id: u32,
        rseq_cs: u64,
        flags: u32,
    }

    #[thread_local]
    static RSEQ: Cell<*mut rseq> = Cell::new(ptr::null_mut());

    #[inline(never)]
    fn get_rseq_slow() -> *mut rseq {
        extern {
            static __rseq_offset: usize;
        }
        let tp: *mut u8;
        let rseq: *mut rseq;
        unsafe {
            asm!("movq %fs:0, {tp}", tp = out(reg) tp, options(att_syntax));
            rseq = tp.add(__rseq_offset) as *mut rseq;
        }
        RSEQ.set(rseq);
        rseq
    }

    #[inline(always)]
    fn get_rseq() -> *mut rseq {
        let mut rseq = RSEQ.get();

        if rseq.is_null() {
            return get_rseq_slow();
        }

        rseq
    }

    #[inline(always)]
    pub unsafe fn with_rseq<T, F>(rseq_cs: *const u8, f: F) -> T
        where F: FnOnce(&u32) -> T
    {
        let rseq = &mut *get_rseq();
        rseq.rseq_cs = rseq_cs as usize as u64;
        let res = f(&rseq.cpu_id);
        rseq.rseq_cs = 0;
        res
    }
}
