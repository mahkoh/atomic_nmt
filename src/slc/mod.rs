use std::sync::atomic::AtomicUsize;
use {
    crate::nmt::{
        inner::Inner,
        versioning::{Versioned, VersioningU64},
    },
    std::sync::{atomic::Ordering::Relaxed, Arc},
};
use crate::nmt::inner::get_cpu;

#[derive(Clone)]
pub struct AtomicSlc<T: Send + Sync> {
    cached: Versioned<VersioningU64, T>,
    inner: Arc<Inner<VersioningU64, T>>,
}

impl<T> AtomicSlc<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            cached: Versioned {
                version: 0,
                value: value.clone(),
            },
            inner: Arc::new(Inner::new(value)),
        }
    }

    #[cold]
    fn maybe_update_slow(&mut self) {
        // self.cached = self.inner.value.lock().clone();
        self.cached = self.inner.get();
    }

    pub fn get(&mut self) -> &T {
        unsafe {
            if self.inner.version.0.load(Relaxed) > self.cached.version {
                // static COUNT: AtomicUsize = AtomicUsize::new(1);
                // println!("updated {}", COUNT.fetch_add(1, Relaxed));
                self.maybe_update_slow();
            }
        }
        &self.cached.value
    }

    pub fn set(&mut self, value: T) {
        let version = self.inner.set(value.clone());
        self.cached.value = value;
        self.cached.version = version;
    }
}
