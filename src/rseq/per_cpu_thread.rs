use {
    crate::rseq::{abort_on_panic::AbortOnPanic, num_cpus::NUM_CPUS},
    flume::{Receiver, Sender},
    once_cell::sync::Lazy,
    parking_lot::Mutex,
    std::{mem, thread},
};

const BITS_PER_USIZE: usize = mem::size_of::<usize>() * 8;

pub type GcTask = Box<dyn FnOnce() + Send>;

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
    let _abort = AbortOnPanic;

    // Ensure that this function runs only on cpu `cpu`.
    let idx = cpu / BITS_PER_USIZE;
    let offset = cpu % BITS_PER_USIZE;
    let mut items = vec![0; idx + 1];
    items[idx] = 1 << offset;
    sched_setaffinity(0, &items);

    // Run all tasks
    while let Ok(task) = rx.recv() {
        task();
    }
}

fn create_cpu_thread(cpu: usize) -> Sender<GcTask> {
    let (tx, rx) = flume::unbounded();
    thread::Builder::new()
        .name(format!("atomic per-cpu thread {}", cpu))
        .spawn(move || cpu_thread(cpu, rx))
        .expect("Could not spawn thread");
    tx
}

#[repr(align(64))]
struct CpuThread {
    sender: Sender<GcTask>,
}

static THREADS: Lazy<Box<[Mutex<Option<CpuThread>>]>> = Lazy::new(|| {
    std::iter::repeat_with(Default::default)
        .take(*NUM_CPUS)
        .collect()
});

pub fn run_on_cpu(cpu: usize, task: GcTask) {
    let mut thread = THREADS[cpu].lock();
    if thread.is_none() {
        *thread = Some(CpuThread {
            sender: create_cpu_thread(cpu),
        });
    }
    let _ = thread.as_ref().unwrap().sender.send(task);
}
