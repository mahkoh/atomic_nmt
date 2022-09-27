use {once_cell::sync::Lazy, std::str::FromStr};

const PATH: &str = "/sys/devices/system/cpu/possible";

/// Computes the highest possible index of a CPU in this system plus 1.
///
/// This value is a boot-time setting that does not change until reboot. It is used by the kernel
/// for per-cpu data structures.
///
/// Note: If the process is migrated to a different system with a different value, the behavior
/// is undefined. This is a /proc/self/mem situation.
pub static NUM_CPUS: Lazy<usize> = Lazy::new(|| {
    let possible = match std::fs::read_to_string(PATH) {
        Ok(p) => p,
        Err(e) => panic!("Could not read {}: {}", PATH, e),
    };
    let possible = possible.trim();
    let last = possible.rsplit(',').next().unwrap();
    let last = last.rsplit('-').next().unwrap();
    match usize::from_str(last) {
        Ok(l) => l + 1,
        Err(e) => panic!("Could not parse {}: {}", last, e),
    }
});
