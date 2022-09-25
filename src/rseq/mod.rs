#![allow(non_upper_case_globals, non_camel_case_types, improper_ctypes)]

pub use atomic::Inner;

mod abort_on_panic;
mod atomic;
mod num_cpus;
mod per_cpu_rc;
mod per_cpu_thread;
mod rseq;
