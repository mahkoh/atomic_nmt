use parking_lot::Mutex;

pub struct LazyTransform<T, S, F> {
    transform_fn: F,
    source: Mutex<Option<S>>,
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
        *self.source.lock() = Some(source);
    }

    pub fn get_value(&self) -> Option<T> {
        if let Some(source) = self.source.lock().take() {
            *self.value.lock() = (self.transform_fn)(source);
        }
        self.value.lock().clone()
    }
}
