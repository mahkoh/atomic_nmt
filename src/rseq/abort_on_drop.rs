/// Object that aborts the process when it is dropped. Usually because of panic=unwind.
pub struct AbortOnDrop;

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        std::process::abort();
    }
}
