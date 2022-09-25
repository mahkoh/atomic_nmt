use std::arch::global_asm;

//////////////////////////////////////
// ACQUIRE
//////////////////////////////////////

// language=asm
global_asm!(
    r#"
    .global lazy_atomic_acquire_thread_pointer
	.section .text.lazy_atomic_acquire_thread_pointer,"x",@progbits
	.align 32
lazy_atomic_acquire_thread_pointer:
    leaq lazy_atomic_acquire_thread_pointer_rseq_cs(%rip), %rax     # %rax = &lazy_atomic_acquire_thread_pointer_rseq_cs
    movq %rax, 8(%rdi)                                              # rseq.rseq_cs = %rax
lazy_atomic_acquire_thread_pointer_start_ip:
    movl 4(%rdi), %eax                                              # %rax = rseq.cpu_id
    movq %rax, %rcx                                                 # %rcx = %rax
    shlq $6, %rcx                                                   # %rcx *= 64;
    movq (%rsi,%rcx), %rdx                                          # %rdx = data_by_cpu[%rax].get()
    testq %rdx, %rdx                                                # test %rdx == null
    je lazy_atomic_acquire_thread_pointer_post_commit_ip            # if true: jump to return
    incq (%rdx)                                                     # %rdx.rc += 1
lazy_atomic_acquire_thread_pointer_post_commit_ip:
    retq                                                            # return (%rax, %rdx)

    # Magic number that must appear immediately before abort_ip. This value is set by glibc
    # when it registers the rseq structure with the kernel. See glibc/sysdeps/unix/sysv/linux/x86/bits/rseq.h
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
lazy_atomic_acquire_thread_pointer_abort_ip:
    jmp lazy_atomic_acquire_thread_pointer
    .size lazy_atomic_acquire_thread_pointer, . - lazy_atomic_acquire_thread_pointer
"#,
    options(att_syntax)
);

/// ```
/// static lazy_atomic_acquire_thread_pointer_rseq_cs: rseq_cs = rseq_cs { ... };
/// ```
// language=asm
global_asm!(
    r#"
	.section .rodata.lazy_atomic_acquire_thread_pointer_rseq_cs,"a",@progbits
	.align 32
lazy_atomic_acquire_thread_pointer_rseq_cs:
    .long 0
    .long 0
    .quad lazy_atomic_acquire_thread_pointer_start_ip
    .quad lazy_atomic_acquire_thread_pointer_post_commit_ip - lazy_atomic_acquire_thread_pointer_start_ip
    .quad lazy_atomic_acquire_thread_pointer_abort_ip

"#,
    options(att_syntax)
);

//////////////////////////////////////
// RELEASE
//////////////////////////////////////

// language=asm
global_asm!(
    r#"
    .global lazy_atomic_release_thread_pointer
	.section .text.lazy_atomic_release_thread_pointer,"x",@progbits
	.align 32
lazy_atomic_release_thread_pointer:
    leaq lazy_atomic_release_thread_pointer_rseq_cs(%rip), %rax     # %rax = &lazy_atomic_release_thread_pointer_rseq_cs
    movq %rax, 8(%rdi)                                              # rseq.rseq_cs = %rax
lazy_atomic_release_thread_pointer_start_ip:
    movl 4(%rdi), %ecx                                              # %rcx = rseq.cpu_id
    movl $2, %eax                                                   # %rax = OFF_CPU
    cmpl 8(%rsi), %ecx                                              # test %rcx == data.cpu_id
    jne lazy_atomic_release_thread_pointer_post_commit_ip           # if false: jump to return
    movq (%rsi), %rcx                                               # %rcx = data.rc
    xorl %eax, %eax                                                 # %rax = ALIVE
    cmpq $1, %rcx                                                   # test %rcx == 1
    sete %al                                                        # if true: %rax = DEAD
    decq %rcx                                                       # %rcx -= 1
    movq %rcx, (%rsi)                                               # data.rc = %rcx
lazy_atomic_release_thread_pointer_post_commit_ip:
    retq                                                            # return %rax

    # See above.
    .ascii "\x0f\xb9\x3d\x53\x30\x05\x53"
lazy_atomic_release_thread_pointer_abort_ip:
    jmp lazy_atomic_release_thread_pointer
    .size lazy_atomic_release_thread_pointer, . - lazy_atomic_release_thread_pointer
"#,
    options(att_syntax)
);

/// ```
/// static lazy_atomic_release_thread_pointer_rseq_cs: rseq_cs = rseq_cs { ... };
/// ```
// language=asm
global_asm!(
    r#"
	.section .rodata.lazy_atomic_release_thread_pointer_rseq_cs,"a",@progbits
	.align 32
lazy_atomic_release_thread_pointer_rseq_cs:
    .long 0
    .long 0
    .quad lazy_atomic_release_thread_pointer_start_ip
    .quad lazy_atomic_release_thread_pointer_post_commit_ip - lazy_atomic_release_thread_pointer_start_ip
    .quad lazy_atomic_release_thread_pointer_abort_ip

"#,
    options(att_syntax)
);
