use parking_lot::Mutex;
use std::ptr;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

pub struct LazyTransform<T, S, F> {
    transform_fn: F,
    source: AtomicPtr<S>,
    value: Mutex<Option<T>>,
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
                *self.value.lock() = (self.transform_fn)(*source);
            }
        }
        self.value.lock().clone()
    }
}
