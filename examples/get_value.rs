use lazy_atomic::AtomicNmt;

fn main() {
    let a = AtomicNmt::new(1);
    loop {
        get_value(&a);
    }
}

#[inline(never)]
#[no_mangle]
fn get_value(a: &AtomicNmt<u8>) -> u8 {
    a.get()
}
