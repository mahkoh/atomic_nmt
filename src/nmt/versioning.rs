use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub trait Versioning: 'static {
    type Version: Copy + Sync + Send + 'static;
    type AtomicVersion: Sync + Send + 'static;

    fn new() -> Self::Version;
    fn new_atomic() -> Self::AtomicVersion;
    fn inc(version: Self::Version) -> Self::Version;
    fn get(version: &Self::AtomicVersion) -> Self::Version;
    fn set(atomic: &Self::AtomicVersion, version: Self::Version);
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
    fn inc(_version: Self::Version) -> Self::Version {
        ()
    }

    #[inline(always)]
    fn get(_version: &Self::AtomicVersion) -> Self::Version {
        ()
    }

    #[inline(always)]
    fn set(_atomic: &Self::AtomicVersion, _version: Self::Version) {
        // nothing
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
    fn inc(version: Self::Version) -> Self::Version {
        version + 1
    }

    #[inline(always)]
    fn get(version: &Self::AtomicVersion) -> Self::Version {
        version.load(Relaxed)
    }

    #[inline(always)]
    fn set(atomic: &Self::AtomicVersion, version: Self::Version) {
        atomic.store(version, Relaxed);
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
