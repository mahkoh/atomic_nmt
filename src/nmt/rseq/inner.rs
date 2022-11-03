use {
    crate::nmt::{
        inner::{
            cache_line::CacheLineAligned,
            num_cpus::NUM_CPUS,
            per_cpu_rc::{self, PerCpuRc},
            rseq::{self, get_rseq},
        },
        versioning::{Versioned, Versioning},
    },
    parking_lot::Mutex,
    std::{
        iter,
        ops::Deref,
        ptr,
        sync::{
            atomic::{
                AtomicPtr,
                Ordering::{AcqRel, Acquire, Relaxed},
            },
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
    pub fn set(self: &Arc<Self>, value: T) {
        let mut new: Box<_> = (0..*NUM_CPUS)
            .map(|cpu_id| {
                let value = Versioned {
                    version: V::new(),
                    value: value.clone(),
                };
                per_cpu_rc::new(cpu_id as _, value)
            })
            .collect();
        if let Some(_lock) = self.set_lock.0.try_lock() {
            let version = V::inc(V::get(&self.version.0));
            for i in 0..*NUM_CPUS {
                unsafe {
                    (*new[i]).value.version = version;
                }
                new[i] = self.new_value_by_cpu[i].0.swap(new[i], AcqRel);
            }
            V::set(&self.version.0, version);
        }
        for &old in new.deref() {
            if !old.is_null() {
                unsafe {
                    drop(Box::from_raw(old));
                }
            }
        }
    }

    #[inline]
    unsafe fn maybe_update(&self, rseq: *mut rseq::rseq) {
        let cpu = (*rseq).cpu_id as usize;
        let new = self.new_value_by_cpu.get_unchecked(cpu);
        if new.0.load(Relaxed).is_null() {
            return;
        }
        let new = new.0.swap(ptr::null_mut(), Acquire);
        if new.is_null() {
            return;
        }
        let old = self.value_by_cpu.get_unchecked(cpu).0.swap(new, AcqRel);
        per_cpu_rc::release(rseq, &*old);
    }

    #[inline]
    pub fn get(self: &Arc<Self>) -> Versioned<V, T> {
        unsafe {
            let rseq = get_rseq();
            self.maybe_update(rseq);
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
