use {
    crate::nmt::inner::{
        abort_on_drop::AbortOnDrop, cache_line::CacheLineAligned, num_cpus::NUM_CPUS,
    },
    flume::{Receiver, Sender},
    once_cell::sync::Lazy,
    parking_lot::Mutex,
    std::{mem, thread},
};
use crate::set_priority;

const BITS_PER_USIZE: usize = mem::size_of::<usize>() * 8;

pub type GcTask = Box<dyn FnOnce() + Send>;

/// See https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html
fn sched_setaffinity(pid: libc::pid_t, mask: &[usize]) {
    unsafe {
        let res = libc::syscall(
            libc::SYS_sched_setaffinity,
            pid as usize,
            mem::size_of_val(mask) as usize,
            mask.as_ptr() as usize,
        );
        if res == -1 {
            panic!(
                "Could not set affinity: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

fn cpu_thread(cpu: usize, rx: Receiver<GcTask>) {
    // set_priority(1);

    // Not strictly necessary but we'll OOM if the thread dies anyway.
    let _abort = AbortOnDrop;

    // Ensure that this function runs only on cpu `cpu`.
    let idx = cpu / BITS_PER_USIZE;
    let offset = cpu % BITS_PER_USIZE;
    let mut items = vec![0; idx + 1];
    items[idx] = 1 << offset;
    sched_setaffinity(0, &items);

    // NOTE: If this cpu is unplugged at runtime, the kernel automatically changes the
    // affinity mask back to the default. In this case the code below will likely cause
    // memory corruption. No easy way to prevent this. I consider this a /proc/self/mem
    // situation.

    // Run all tasks
    loop {
        rx.recv().unwrap()();
    }
}

fn create_cpu_thread(cpu: usize) -> Sender<GcTask> {
    let (tx, rx) = flume::unbounded();
    thread::Builder::new()
        // NOTE: Maximum length is 15 bytes. The maximum is therefore `la-per-cpu 9999`.
        .name(format!("la-per-cpu {}", cpu))
        .spawn(move || cpu_thread(cpu, rx))
        .expect("Could not spawn thread");
    tx
}

struct CpuThread {
    sender: Sender<GcTask>,
    _aligned: CacheLineAligned<()>,
}

static THREADS: Lazy<Box<[Mutex<Option<CpuThread>>]>> = Lazy::new(|| {
    std::iter::repeat_with(Default::default)
        .take(*NUM_CPUS)
        .collect()
});

/// Runs the task on the specified CPU.
pub fn run_on_cpu(cpu: usize, task: GcTask) {
    let mut thread = THREADS[cpu].lock();
    if thread.is_none() {
        *thread = Some(CpuThread {
            sender: create_cpu_thread(cpu),
            _aligned: Default::default(),
        });
    }
    let _ = thread.as_ref().unwrap().sender.send(task);
}
