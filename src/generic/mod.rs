use {
    parking_lot::Mutex,
    std::{mem, ops::DerefMut},
};

struct Inner<T> {
    value: Mutex<T>,
}

impl<T> Inner<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Mutex::new(value),
        }
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.value.lock().clone()
    }

    pub fn set(&self, value: T) {
        let _v = mem::replace(self.value.lock().deref_mut(), value);
    }
}
