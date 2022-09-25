use {cfg_if::cfg_if, std::sync::Arc};

cfg_if! {
    if #[cfg(all(target_os = "linux", any(target_arch = "x86_64")))] {
        use rseq::Inner;
        mod rseq;
    } else {
        use generic::Inner;
        mod generic;
    }
}

pub struct Atomic<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Clone for Atomic<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Atomic<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Inner::new(value)),
        }
    }

    #[inline]
    pub fn set(&self, value: T) {
        self.inner.set(value);
    }

    #[inline]
    pub fn get(&self) -> T {
        self.inner.get()
    }
}
