use parking_lot::Mutex;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::{iter, ptr};

pub struct LazyTransform<T, S, F> {
    transform_fn: F,
    source: AtomicPtr<S>,
    cpu_value: Vec<Mutex<Option<T>>>,
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
            cpu_value: iter::repeat_with(Default::default)
                .take(num_cpus::get())
                .collect(),
        }
    }

    pub fn set_source(&self, source: S) {
        self.source.store(Box::into_raw(Box::new(source)), Release);
    }

    pub fn get_value(&self) -> Option<T> {
        if !self.source.load(Relaxed).is_null() {
            let source = self.source.swap(ptr::null_mut(), Acquire);
            if !source.is_null() {
                let source = unsafe { Box::from_raw(source) };
                let value = (self.transform_fn)(*source);
                for cpu in &self.cpu_value {
                    *cpu.lock() = value.clone();
                }
            }
        }
        self.cpu_value[getcpu()].lock().clone()
    }
}

fn getcpu() -> usize {
    unsafe {
        let cpu = libc::sched_getcpu();
        assert!(cpu >= 0);
        cpu as _
    }
}
