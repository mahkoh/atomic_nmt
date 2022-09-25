/// Type that is cache-line aligned.
///
/// This type is used to prevent false sharing. False sharing is when two different threads
/// access different variables that live in the same cache line and where at least one of
/// those two accesses is a write. This causes the cache line to be locked by the writing
/// thread, forcing the other thread to wait.
///
/// By aligning variables at cache lines, we can ensure that they live in different cache
/// lines.
#[cfg_attr(any(target_arch = "x86_64"), repr(C, align(64)))]
#[derive(Default)]
pub struct CacheLineAligned<T>(pub T);

impl<T> From<T> for CacheLineAligned<T> {
    fn from(v: T) -> Self {
        Self(v)
    }
}
