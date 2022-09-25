use std::arch::global_asm;

//////////////////////////////////////
// ACQUIRE
//////////////////////////////////////

/// ```
/// use std::cell::Cell;
/// unsafe extern fn lazy_transform_acquire_thread_pointer(
///     rseq: *mut rseq,
///     data_by_cpu: &[Cell<*mut PerCpuRc<u8>>],
/// ) -> (u32, *mut PerCpuRc<u8>) {
///     (*rseq).rseq_cs = &lazy_transform_acquire_thread_pointer_rseq_cs as *const _ as u64;
///     let cpu = (*rseq).cpu_id;
///     let data = *data_by_cpu.get_unchecked(cpu as usize).get();
///     if !data.is_null() {
///         (*data).rc += 1;
///     }
///     (cpu, data)
/// }
/// ```
// language=asm
global_asm!(
    r#"
    .global lazy_transform_acquire_thread_pointer
	.section .text.lazy_transform_acquire_thread_pointer,"x",@progbits
	.align 32
lazy_transform_acquire_thread_pointer:
    leaq lazy_transform_acquire_thread_pointer_rseq_cs(%rip), %rax  # %rax = &lazy_transform_acquire_thread_pointer_rseq_cs
    movq %rax, 8(%rdi)                                              # rseq.rseq_cs = %rax
lazy_transform_acquire_thread_pointer_start_ip:
    movl 4(%rdi), %eax                                              # %rax = rseq.cpu_id
    movq (%rsi,%rax,8), %rdx                                        # %rdx = data_by_cpu[%rax].get()
    testq %rdx, %rdx                                                # test %rdx == null
    je lazy_transform_acquire_thread_pointer_post_commit_ip         # if true: jump to return
    incq (%rdx)                                                     # %rdx.rc += 1
lazy_transform_acquire_thread_pointer_post_commit_ip:
    retq                                                            # return (%rax, %rdx)

    # magic number that must appear immediately before abort_ip
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
lazy_transform_acquire_thread_pointer_abort_ip:
    jmp lazy_transform_acquire_thread_pointer
    .size lazy_transform_acquire_thread_pointer, . - lazy_transform_acquire_thread_pointer
"#,
    options(att_syntax)
);

/// ```
/// static lazy_transform_acquire_thread_pointer_rseq_cs: rseq_cs = rseq_cs {
///     version: 0,
///     flags: 0,
///     start_ip: lazy_transform_acquire_thread_pointer_start_ip,
///     post_commit_offset: lazy_transform_acquire_thread_pointer_post_commit_ip - lazy_transform_acquire_thread_pointer_start_ip,
///     abort_ip: lazy_transform_acquire_thread_pointer_abort_ip,
/// };
/// ```
// language=asm
global_asm!(
    r#"
	.section .rodata.lazy_transform_acquire_thread_pointer_rseq_cs,"a",@progbits
	.align 32
lazy_transform_acquire_thread_pointer_rseq_cs:
    .long 0
    .long 0
    .quad lazy_transform_acquire_thread_pointer_start_ip
    .quad lazy_transform_acquire_thread_pointer_post_commit_ip - lazy_transform_acquire_thread_pointer_start_ip
    .quad lazy_transform_acquire_thread_pointer_abort_ip

"#,
    options(att_syntax)
);

//////////////////////////////////////
// RELEASE
//////////////////////////////////////

/// ```
/// unsafe extern fn lazy_transform_release_thread_pointer(
///     rseq: *mut rseq,
///     data: *mut PerCpuRc<u8>,
/// ) -> u64 {
///     (*rseq).rseq_cs = &lazy_transform_release_thread_pointer_rseq_cs as *const _ as u64;
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
// language=asm
global_asm!(
    r#"
    .global lazy_transform_release_thread_pointer
	.section .text.lazy_transform_release_thread_pointer,"x",@progbits
	.align 32
lazy_transform_release_thread_pointer:
    leaq lazy_transform_release_thread_pointer_rseq_cs(%rip), %rax  # %rax = &lazy_transform_release_thread_pointer_rseq_cs
    movq %rax, 8(%rdi)                                              # rseq.rseq_cs = %rax
lazy_transform_release_thread_pointer_start_ip:
    movl 4(%rdi), %ecx                                              # %rcx = rseq.cpu_id
    movl $2, %eax                                                   # %rax = OFF_CPU
    cmpl 8(%rsi), %ecx                                              # test %rcx == data.cpu_id
    jne lazy_transform_release_thread_pointer_post_commit_ip        # if false: jump to return
    movq (%rsi), %rcx                                               # %rcx = data.rc
    xorl %eax, %eax                                                 # %rax = ALIVE
    cmpq $1, %rcx                                                   # test %rcx == 1
    sete %al                                                        # if true: %rax = DEAD
    decq %rcx                                                       # %rcx -= 1
    movq %rcx, (%rsi)                                               # data.rc = %rcx
lazy_transform_release_thread_pointer_post_commit_ip:
    retq                                                            # return %rax

    # magic number that must appear immediately before abort_ip
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
lazy_transform_release_thread_pointer_abort_ip:
    jmp lazy_transform_release_thread_pointer
    .size lazy_transform_release_thread_pointer, . - lazy_transform_release_thread_pointer
"#,
    options(att_syntax)
);

/// ```
/// static lazy_transform_release_thread_pointer_rseq_cs: rseq_cs = rseq_cs {
///     version: 0,
///     flags: 0,
///     start_ip: lazy_transform_release_thread_pointer_start_ip,
///     post_commit_offset: lazy_transform_release_thread_pointer_post_commit_ip - lazy_transform_release_thread_pointer_start_ip,
///     abort_ip: lazy_transform_release_thread_pointer_abort_ip,
/// };
/// ```
// language=asm
global_asm!(
    r#"
	.section .rodata.lazy_transform_release_thread_pointer_rseq_cs,"a",@progbits
	.align 32
lazy_transform_release_thread_pointer_rseq_cs:
    .long 0
    .long 0
    .quad lazy_transform_release_thread_pointer_start_ip
    .quad lazy_transform_release_thread_pointer_post_commit_ip - lazy_transform_release_thread_pointer_start_ip
    .quad lazy_transform_release_thread_pointer_abort_ip

"#,
    options(att_syntax)
);
