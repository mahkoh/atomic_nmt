mod bench_arc;

use {
    lazy_transform::Atomic,
    std::{sync::Arc, thread, time::Duration},
};

#[used]
#[no_mangle]
static mut XXXXXXXXXXXXXXXXXXX: i32 = 0;

fn main() {
    let atomic = Atomic::new(Arc::new(123));
    {
        let atomic = atomic.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            atomic.set(Arc::new(456));
        });
    }
    let mut prev = atomic.get();
    for i in 1.. {
        let value = atomic.get();
        if value != prev {
            println!("changed {} -> {}", prev, value);
            prev = value;
        }
        if i % 1_000_000 == 0 {
            println!("{}", i);
        }
    }
}

#[inline(never)]
#[no_mangle]
fn get_value(atomic: &Atomic<i32>) -> i32 {
    atomic.get()
}
