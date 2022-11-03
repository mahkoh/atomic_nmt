#![allow(non_upper_case_globals, non_camel_case_types, improper_ctypes)]

pub use inner::Inner;

mod abort_on_drop;
mod cache_line;
mod inner;
mod num_cpus;
mod per_cpu_rc;
pub mod per_cpu_thread;
mod rseq;
