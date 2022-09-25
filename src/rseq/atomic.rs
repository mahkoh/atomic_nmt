use {
    crate::rseq::{
        num_cpus::NUM_CPUS,
        per_cpu_rc::{self, PerCpuRc},
        per_cpu_thread::run_on_cpu,
    },
    parking_lot::Mutex,
    std::{
        cell::Cell,
        iter, ptr,
        sync::{
            atomic::{AtomicBool, Ordering::Relaxed},
            Arc,
        },
    },
};

pub struct Inner<T> {
    pub value: Mutex<T>,
    pub updating_cpu_value: Box<[AtomicBool]>,
    pub cpu_init: Box<[AtomicBool]>,
    pub value_by_cpu: Box<[Cell<*mut PerCpuRc<T>>]>,
}

unsafe impl<T: Send + Sync + 'static> Send for Inner<T> {}
unsafe impl<T: Send + Sync + 'static> Sync for Inner<T> {}

impl<T> Inner<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            value: Mutex::new(value),
            updating_cpu_value: iter::repeat_with(|| AtomicBool::new(true))
                .take(*NUM_CPUS)
                .collect(),
            cpu_init: iter::repeat_with(|| AtomicBool::new(false))
                .take(*NUM_CPUS)
                .collect(),
            value_by_cpu: iter::repeat_with(|| Cell::new(ptr::null_mut()))
                .take(*NUM_CPUS)
                .collect(),
        }
    }

    #[inline]
    pub fn set(self: &Arc<Self>, value: T) {
        *self.value.lock() = value;
        for cpu_id in 0..*NUM_CPUS {
            self.update_cpu_value(cpu_id);
        }
    }

    fn update_cpu_value(self: &Arc<Self>, cpu_id: usize) {
        if self.updating_cpu_value[cpu_id].load(Relaxed) {
            return;
        }
        if self.updating_cpu_value[cpu_id].swap(true, Relaxed) {
            return;
        }
        let inner = self.clone();
        run_on_cpu(
            cpu_id,
            Box::new(move || {
                inner.updating_cpu_value[cpu_id].store(false, Relaxed);
                let value = inner.value.lock().clone();
                let rc = per_cpu_rc::new(cpu_id as _, value);
                let old = inner.value_by_cpu[cpu_id].replace(rc);
                if !old.is_null() {
                    unsafe {
                        per_cpu_rc::release(&*old);
                    }
                }
            }),
        );
    }

    #[inline]
    pub fn get(self: &Arc<Self>) -> T {
        unsafe {
            let (cpu_id, rc) = per_cpu_rc::acquire(&self.value_by_cpu);
            if let Some(rc) = rc {
                let value = rc.value.clone();
                per_cpu_rc::release(rc);
                return value;
            }
            self.get_init(cpu_id)
        }
    }

    #[inline(never)]
    #[cold]
    fn get_init(self: &Arc<Self>, cpu_id: usize) -> T {
        if !self.cpu_init[cpu_id].swap(true, Relaxed) {
            self.updating_cpu_value[cpu_id].store(false, Relaxed);
            self.update_cpu_value(cpu_id);
        }
        self.value.lock().clone()
    }
}
