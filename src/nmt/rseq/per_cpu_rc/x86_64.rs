use {
    crate::nmt::inner::{cache_line::CacheLineAligned, per_cpu_rc::PerCpuRc, rseq::rseq},
    std::{arch::asm, cell::Cell},
};

/// ```no_run
/// use std::cell::Cell;
/// unsafe fn acquire(
///     rseq: *mut rseq,
///     data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<u8>>>],
/// ) -> &PerCpuRc<u8> {
///     let cpu = (*rseq).cpu_id;
///     let data = &mut **data_by_cpu.get_unchecked(cpu as usize).0.get();
///     data.rc += 1;
///     data
/// }
/// ```
#[inline]
pub unsafe fn acquire<T: Send + Sync>(
    rseq: *mut rseq,
    data_by_cpu: &[CacheLineAligned<Cell<*mut PerCpuRc<T>>>],
) -> &PerCpuRc<T> {
    let data: *const PerCpuRc<T>;
    asm!(
        r#"
1:
    leaq 5f(%rip), {data}
    movq {data}, 8({rseq})
2:
    movl 4({rseq}), {data:e}
    shlq $6, {data}
    movq ({data_by_cpu},{data}), {data}
    incq ({data})
3:
    jmp 6f

    # Magic number that must appear immediately before abort_ip. This value is set by glibc
    # when it registers the rseq structure with the kernel. See glibc/sysdeps/unix/sysv/linux/x86/bits/rseq.h
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
4:
    jmp 1b

5:
    .long 0
    .long 0
    .quad 2b
    .quad 3b - 2b
    .quad 4b

6:
"#,
        rseq = in(reg) rseq,
        data_by_cpu = in(reg) data_by_cpu.as_ptr(),
        data = out(reg) data,
        options(att_syntax),
    );
    &*data
}

/// ```
/// unsafe fn release(
///     rseq: *mut rseq,
///     data: *mut PerCpuRc<u8>,
/// ) -> u64 {
///     let cpu = (*rseq).cpu_id;
///     let mut res = OFF_CPU;
///     if cpu == (*data).cpu_id {
///         res = ALIVE;
///         if (*data).rc == 1 {
///             res = DEAD;
///         }
///         (*data).rc -= 1;
///     }
///     res
/// }
/// ```
#[inline]
pub unsafe fn release<T: Send + Sync>(rseq: *mut rseq, data: *mut PerCpuRc<T>) -> u64 {
    let res: u64;
    asm!(
        r#"
1:
    leaq 5f(%rip), {tmp}
    movq {tmp}, 8({rseq})
2:
    movl 4({rseq}), {tmp:e}
    movl $2, {res:e}
    cmpl 8({data}), {tmp:e}
    jne 6f
    movq ({data}), {tmp}
    xorl {res:e}, {res:e}
    cmpq $1, {tmp}
    sete {res:l}
    decq {tmp}
    movq {tmp}, ({data})
3:
    jmp 6f

    # See above.
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
4:
    jmp 1b

5:
    .long 0
    .long 0
    .quad 2b
    .quad 3b - 2b
    .quad 4b

6:
"#,
        rseq = in(reg) rseq,
        data = in(reg) data,
        tmp = out(reg) _,
        res = out(reg) res,
        options(att_syntax)
    );
    res
}
