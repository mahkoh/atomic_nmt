use std::thread;
use lazy_atomic::Atomic;

/// This example shows that later calls to `get` can return earlier values.
fn main() {
    let atomic = Atomic::new(0);
    {
        let atomic = atomic.clone();
        thread::spawn(move || {
            for i in 1u64.. {
                atomic.set(i);
            }
        });
    }
    loop {
        let mut first = atomic.get();
        let mut second = atomic.get();
        if second < first {
            println!("{first} -> {second}");
        }
    }
}
