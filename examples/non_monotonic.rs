use {lazy_atomic::AtomicNmt, std::thread};

/// This example shows that later calls to `get` can return earlier values.
fn main() {
    let atomic = AtomicNmt::new(0);
    {
        let atomic = atomic.clone();
        thread::spawn(move || {
            for i in 1u64.. {
                atomic.set(i);
            }
        });
    }
    loop {
        let first = atomic.get();
        let second = atomic.get();
        if second < first {
            println!("{first} -> {second}");
        }
    }
}
