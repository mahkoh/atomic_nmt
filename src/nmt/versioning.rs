use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub trait Versioning: 'static {
    type Version: Copy + Sync + Send + 'static;
    type AtomicVersion: Sync + Send + 'static;

    fn new() -> Self::Version;
    fn new_atomic() -> Self::AtomicVersion;
    fn inc(version: &mut Self::Version);
    fn set(atomic: &Self::AtomicVersion, version: Self::Version);
    fn is_above(value: Self::Version, bound: Self::Version) -> bool;
}

pub struct VersioningNone;

impl Versioning for VersioningNone {
    type Version = ();
    type AtomicVersion = ();

    fn new() -> Self::Version {
        ()
    }

    fn new_atomic() -> Self::AtomicVersion {
        ()
    }

    #[inline(always)]
    fn inc(_version: &mut Self::Version) {
        // nothing
    }
    #[inline(always)]
    fn set(_atomic: &Self::AtomicVersion, _version: Self::Version) {
        // nothing
    }

    #[inline(always)]
    fn is_above(_value: Self::Version, _bound: Self::Version) -> bool {
        true
    }
}

pub struct VersioningU64;

impl Versioning for VersioningU64 {
    type Version = u64;
    type AtomicVersion = AtomicU64;

    fn new() -> Self::Version {
        0
    }

    fn new_atomic() -> Self::AtomicVersion {
        AtomicU64::new(0)
    }

    #[inline(always)]
    fn inc(version: &mut Self::Version) {
        *version += 1;
    }

    #[inline(always)]
    fn set(atomic: &Self::AtomicVersion, version: Self::Version) {
        atomic.store(version, Relaxed);
    }

    fn is_above(value: Self::Version, bound: Self::Version) -> bool {
        value > bound
    }
}

pub struct Versioned<V: Versioning, T> {
    pub version: V::Version,
    pub value: T,
}

impl<V: Versioning, T: Clone> Clone for Versioned<V, T> {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            value: self.value.clone(),
        }
    }
}
