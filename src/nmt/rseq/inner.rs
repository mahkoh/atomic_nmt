use std::{mem, ptr};
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use parking_lot::lock_api::RawMutex as _;
use parking_lot::RawMutex;
use {
    crate::nmt::{
        inner::{
            cache_line::CacheLineAligned,
            num_cpus::NUM_CPUS,
            per_cpu_rc::{self, PerCpuRc},
            per_cpu_thread::run_on_cpu,
            rseq::{self, get_rseq},
        },
        versioning::{Versioned, Versioning},
    },
    parking_lot::Mutex,
    std::{
        arch::asm,
        cell::Cell,
        iter,
        sync::{
            atomic::{AtomicBool, Ordering::Relaxed},
            Arc,
        },
    },
};

pub struct Inner<V: Versioning, T: Send + Sync> {
    pub version: CacheLineAligned<V::AtomicVersion>,
    pub set_lock: CacheLineAligned<Mutex<()>>,
    pub value_by_cpu: Box<[CacheLineAligned<AtomicPtr<PerCpuRc<Versioned<V, T>>>>]>,
    pub new_value_by_cpu: Box<[CacheLineAligned<AtomicPtr<PerCpuRc<Versioned<V, T>>>>]>,
}

unsafe impl<V: Versioning, T: Send + Sync + 'static> Send for Inner<V, T> {}
unsafe impl<V: Versioning, T: Send + Sync + 'static> Sync for Inner<V, T> {}

impl<V, T> Inner<V, T>
where
    V: Versioning,
    T: Clone + Send + Sync + 'static,
{
    pub fn new(value: T) -> Self {
        rseq::ensure_enabled();
        let value = Versioned {
            version: V::new(),
            value: value.clone(),
        };
        Self {
            version: V::new_atomic().into(),
            set_lock: Mutex::new(()).into(),
            value_by_cpu: (0..*NUM_CPUS)
                .map(|cpu_id| AtomicPtr::new(per_cpu_rc::new(cpu_id as _, value.clone())).into())
                .collect(),
            new_value_by_cpu: iter::repeat_with(|| AtomicPtr::default().into())
                .take(*NUM_CPUS)
                .collect(),
        }
    }

    #[inline]
    pub fn set(self: &Arc<Self>, value: T) -> V::Version {
        let mut new: Box<_>;
        let version = {
            let _lock = self.set_lock.0.lock();
            let version = V::inc(V::get(&self.version.0));
            let value = Versioned {
                version,
                value,
            };
            new = (0..*NUM_CPUS)
                .map(|cpu_id| per_cpu_rc::new(cpu_id as _, value.clone()))
                .collect();
            for (old, new) in self.new_value_by_cpu.iter().zip(new.iter_mut()) {
                *new = old.0.swap(*new, Release);
            }
            V::set(&self.version.0, version);
            version
        };
        for &old in new.deref() {
            if !old.is_null() {
                unsafe {
                    drop(Box::from_raw(old));
                }
            }
        }
        version
    }

    #[inline]
    unsafe fn update(&self, news: &AtomicPtr<PerCpuRc<Versioned<V, T>>>, rseq: *mut rseq::rseq, cpu: usize) {
        let new = news.swap(ptr::null_mut(), Acquire);
        if !new.is_null() {
            let old = self.value_by_cpu.get_unchecked(cpu).0.swap(new, AcqRel);
            per_cpu_rc::release(rseq, &*old);
        }
    }

    #[inline]
    pub fn get(self: &Arc<Self>) -> Versioned<V, T> {
        unsafe {
            let rseq = get_rseq();
            let cpu = (*rseq).cpu_id as usize;
            let news = self.new_value_by_cpu.get_unchecked(cpu);
            if !news.0.load(Relaxed).is_null() {
                self.update(&news.0, rseq, cpu);
            }
            let rc = per_cpu_rc::acquire(rseq, &self.value_by_cpu);
            let value = rc.value.clone();
            per_cpu_rc::release(rseq, rc);
            value
        }
    }
}

impl<V: Versioning, T: Send + Sync> Drop for Inner<V, T> {
    fn drop(&mut self) {
        let rseq = get_rseq();
        // SAFETY: The destructors of `Arc` synchronize with each other. For each
        // modification of the `value_by_cpu` array (in `update_cpu_value`), the `drop`
        // of the `Arc` though which the array was accessed happened after
        // the modification. Therefore, this `drop` call happens after the value was
        // written to the `value_by_cpu` array. Therefore, this `drop` call sees the
        // latest state of the `value_by_cpu` array and its contents.
        for value in self.value_by_cpu.iter() {
            let value = value.0.load(Acquire);
            unsafe {
                // SAFETY: We're releasing the reference owned by the `value_by_cpu`
                // array.
                per_cpu_rc::release(rseq, &*value);
            }
        }
        for value in self.new_value_by_cpu.iter() {
            let value = value.0.load(Acquire);
            if !value.is_null() {
                unsafe {
                    drop(Box::from_raw(value));
                }
            }
        }
    }
}
