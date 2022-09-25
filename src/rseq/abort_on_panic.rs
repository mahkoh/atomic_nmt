use std::arch::asm;

pub struct AbortOnPanic;

impl Drop for AbortOnPanic {
    fn drop(&mut self) {
        unsafe {
            asm!("ud2");
        }
    }
}
