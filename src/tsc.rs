
/// Returns true if the time stamp counter is supported.
pub fn is_supported() -> bool {
    unsafe {
        core::arch::x86_64::__cpuid(0x00000001).edx & (1 << 4) != 0
    }
}

/// Returns true if the time stamp counter is invariant. Invariant means that it is not affected by
/// frequency changes, nor by the different power states of the CPU.
/// 
/// Please note that this function cannot distinguish between invariant TSCs, and constant TSCs (
/// which can vary in frequency when the CPU is in a low power state).
pub fn is_invariant() -> bool {
    unsafe {
        core::arch::x86_64::__cpuid(0x80000007).edx & (1 << 8) != 0
    }
}

/// Reads the time stamp counter. 
/// 
/// The processor monotonically increments the time-stamp counter MSR every clock cycle and resets
/// it to 0 whenever the processor is reset.
/// The RDTSC instruction is not a serializing instruction. It does not necessarily wait until all 
/// previous instructions have been executed before reading the counter. Similarly, subsequent 
/// instructions may begin execution before the read operation is performed.
pub fn read() -> u64 {
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
}
