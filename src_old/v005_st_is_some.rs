pub struct LazyTransform<T, S, F> {
    transform_fn: F,
    source: Option<S>,
    value: Option<T>,
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

    pub fn set_source(&mut self, source: S) {
        self.source = Some(source);
    }

    pub fn get_value(&mut self) -> Option<T> {
        if self.source.is_some() {
            if let Some(source) = self.source.take() {
                self.value = (self.transform_fn)(source);
            }
        }
        self.value.clone()
    }
}
