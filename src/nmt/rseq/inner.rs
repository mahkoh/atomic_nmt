use std::mem;
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
    /// The latest version of the stored value.
    pub value: Mutex<Versioned<V, T>>,
    /// The numeric version of the value.
    pub version: CacheLineAligned<V::AtomicVersion>,
    /// Usually true if and only if an update of the per-cpu value to the latest value has
    /// been schedule in the per-cpu thread.
    ///
    /// The only exception is that, after the object has been constructed, these values are
    /// all `true` even though nothing has been scheduled. This is so that no additional updates
    /// are scheduled on these CPUs.
    ///
    /// When `get` is called on a CPU for the first time, this value is set to `false` and an
    /// update is scheduled immediately.
    pub updating_cpu_value: Box<[CacheLineAligned<AtomicBool>]>,
    pub version_by_cpu: Box<[CacheLineAligned<V::AtomicVersion>]>,
    /// The per-cpu value. NOTE: Index `i` of this slice is only ever accessed by cpu `i` except
    /// in this types `Drop` implementation. Therefore, we don't ever have to use atomic operations
    /// while accessing it.
    pub value_by_cpu: Box<[CacheLineAligned<Cell<*mut PerCpuRc<Versioned<V, T>>>>]>,
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
            value: Mutex::new(value.clone()),
            version: V::new_atomic().into(),
            updating_cpu_value: iter::repeat_with(|| AtomicBool::new(false).into())
                .take(*NUM_CPUS)
                .collect(),
            version_by_cpu: iter::repeat_with(|| V::new_atomic().into()).take(*NUM_CPUS).collect(),
            value_by_cpu: (0..*NUM_CPUS)
                .map(|cpu_id| Cell::new(per_cpu_rc::new(cpu_id as _, value.clone())).into())
                .collect(),
        }
    }

    #[inline]
    pub fn set(self: &Arc<Self>, value: T) -> V::Version {
        let _old;
        let version = {
            let mut lock = self.value.lock();
            _old = mem::replace(&mut lock.value, value);
            V::inc(&mut lock.version);
            V::set(&self.version.0, lock.version);
            lock.version
        };
        for cpu_id in 0..*NUM_CPUS {
            self.update_cpu_value(cpu_id);
        }
        version
    }

    /// Schedule an update of the CPU-local value. If an update is already scheduled, this
    /// function is a no-op.
    fn update_cpu_value(self: &Arc<Self>, cpu_id: usize) {
        // Optimistically perform a load first.
        if self.updating_cpu_value[cpu_id].0.load(Relaxed) {
            // An update of this cpu's value is already scheduled. This is the fast path.
            return;
        }
        // Try to acquire the permission to update the value.
        if self.updating_cpu_value[cpu_id].0.swap(true, Relaxed) {
            // We raced with another thread and that thread won.
            return;
        }
        let inner = self.clone();
        run_on_cpu(
            cpu_id,
            Box::new(move || {
                inner.updating_cpu_value[cpu_id].0.store(false, Relaxed);
                let value = inner.value.lock().clone();
                let version = value.version;
                let mut rc = per_cpu_rc::new(cpu_id as _, value);
                unsafe {
                    // NOTE: We cannot use Cell::replace here because that function is
                    // implemented as read+write and we might get scheduled in between.
                    // Instead we have to use a single (not necessarily atomic) instruction.
                    #[cfg(target_arch = "x86_64")]
                    asm!(
                        "xchgq {rc}, ({old})",
                        rc = inout(reg) rc,
                        old = in(reg) &inner.value_by_cpu[cpu_id],
                        options(att_syntax)
                    )
                }
                V::set(&inner.version_by_cpu[cpu_id].0, version);
                if !rc.is_null() {
                    unsafe {
                        per_cpu_rc::release(get_rseq(), &*rc);
                    }
                }
            }),
        );
    }

    #[inline]
    pub fn get(self: &Arc<Self>, bound: V::Version) -> Option<Versioned<V, T>> {
        unsafe {
            let rseq = get_rseq();
            let rc = per_cpu_rc::acquire(rseq, &self.value_by_cpu);
            let mut value = None;
            if V::is_above(rc.value.version, bound) {
                value = Some(rc.value.clone());
            }
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
        for value in &*self.value_by_cpu {
            let value = value.0.get();
            if !value.is_null() {
                unsafe {
                    // SAFETY: We're releasing the reference owned by the `value_by_cpu`
                    // array.
                    per_cpu_rc::release(rseq, &*value);
                }
            }
        }
    }
}
