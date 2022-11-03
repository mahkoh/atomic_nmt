pub mod versioning;

use {
    crate::nmt::versioning::VersioningNone,
    cfg_if::cfg_if,
    inner::Inner,
    std::{
        fmt::{Debug, Formatter},
        sync::Arc,
    },
};

cfg_if! {
    if #[cfg(all(target_os = "linux", target_arch = "x86_64"))] {
        // Fast
        #[path = "rseq/mod.rs"]
        pub(crate) mod inner;
    } else {
        // Dogshit
        #[path = "generic/mod.rs"]
        pub(crate) mod inner;
    }
}

/// An atomic variable with eventual consistency.
///
/// This type supports arbitrary `T: Clone + Send + Sync + 'static`.
///
/// Eventual consistency means that, if no new updates a made to the atomic variable,
/// eventually all accesses to it will see the last set value.
///
/// This type does not guarantee monotonicity. See the description of [`Self::get`].
///
/// Currently, only the following targets are supported:
///
/// - linux
///   - x86_64
///
/// On all other targets, this type falls back to `Arc<Mutex<T>>` which will be very slow.
pub struct AtomicNmt<T: Send + Sync> {
    inner: Arc<Inner<VersioningNone, T>>,
}

impl<T> AtomicNmt<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Creates a new `Atomic<T>`.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Inner::new(value)),
        }
    }

    /// Sets the value.
    ///
    /// At some point after this call, all calls to `get` will return this value or a value set by
    /// a later call to `set`.
    #[inline]
    pub fn set(&self, value: T) {
        self.inner.set(value);
    }

    /// Clones the contained value.
    ///
    /// This function does not necessarily return the last value set by `set`. Nor is this function
    /// monotonic:
    ///
    /// ```rust,no_run
    /// atomic.set(1);
    /// atomic.set(2);
    /// assert_eq!(atomic.get(), 2);
    /// assert_eq!(atomic.get(), 2);
    /// ```
    ///
    /// The second assert can fail even if no other threads are accessing `atomic`. However, it is
    /// guaranteed that at some point after the `set(2)` call, all calls to `get` will return
    /// `2`.
    #[inline]
    pub fn get(&self) -> T {
        self.inner.get().value
    }
}

impl<T: Send + Sync> Clone for AtomicNmt<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Debug for AtomicNmt<T>
where
    T: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atomic")
            .field("value", &self.get())
            .finish()
    }
}
