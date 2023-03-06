use core::sync::atomic::{AtomicU64, Ordering};

use crate::address::Virtual;

static LAPIC_BASE: AtomicU64 = AtomicU64::new(0);

/// Represents the local APIC registers. The values are the offsets from the
/// base address of the local APIC.
pub enum Register {
    Id = 0x0020,
    Version = 0x0030,
    TaskPriority = 0x0080,
    ArbitrationPriority = 0x0090,
    ProcessorPriority = 0x00A0,
    EndOfInterrupt = 0x00B0,
    RemoteRead = 0x00C0,
    LogicalDestination = 0x00D0,
    DestinationFormat = 0x00E0,
    SpuriousInterruptVector = 0x00F0,

    InService0 = 0x0100,
    InService1 = 0x0110,
    InService2 = 0x0120,
    InService3 = 0x0130,
    InService4 = 0x0140,
    InService5 = 0x0150,
    InService6 = 0x0160,
    InService7 = 0x0170,

    TriggerMode0 = 0x0180,
    TriggerMode1 = 0x0190,
    TriggerMode2 = 0x01A0,
    TriggerMode3 = 0x01B0,
    TriggerMode4 = 0x01C0,
    TriggerMode5 = 0x01D0,
    TriggerMode6 = 0x01E0,
    TriggerMode7 = 0x01F0,

    InterruptRequest0 = 0x0200,
    InterruptRequest1 = 0x0210,
    InterruptRequest2 = 0x0220,
    InterruptRequest3 = 0x0230,
    InterruptRequest4 = 0x0240,
    InterruptRequest5 = 0x0250,
    InterruptRequest6 = 0x0260,
    InterruptRequest7 = 0x0270,

    ErrorStatus = 0x0280,
    LvtCmci = 0x02F0,
    InterruptCommand0 = 0x0300,
    InterruptCommand1 = 0x0310,
    LvtTimer = 0x0320,
    LvtThermalSensor = 0x0330,
    LvtPerformanceCounter = 0x0340,
    LvtLint0 = 0x0350,
    LvtLint1 = 0x0360,
    LvtError = 0x0370,

    InitialCount = 0x0380,
    CurrentCount = 0x0390,

    DivideConfiguration = 0x03E0,
}

/// Represents the destination of an IPI.
pub enum IpiDestination {
    /// Send the IPI to the given core.
    Core(u8),

    /// Send the IPI to the current core.
    SelfOnly,

    /// Send the IPI to all cores, including the current one.
    AllCores,

    /// Send the IPI to all cores, except the current one.
    OtherCores,
}

pub enum IpiPriority {
    /// Normal priority.
    Normal = 0,

    /// Low priority.
    Low = 1,

    /// ???
    Smi = 2,

    /// NMI priority. Send an NMI instead of an IPI, the interrupt vector is ignored.
    Nmi = 4,
    // ...
}

/// Setup the local APIC. This function must be called before any other function in this module.
/// The parameter is the base virtual address of the local APIC.
///
/// # Safety
/// This function is unsafe because the caller must ensure that the given base address is valid,
/// and is a virtual address that points to the local APIC (and not a physical address !). When
/// remapping the physical memory, caching should be disabled for the local APIC memory region.
pub unsafe fn setup(base: Virtual) {
    assert!(base.is_page_aligned());
    LAPIC_BASE.store(base.as_u64(), Ordering::Relaxed);
}

/// Enable the local APIC by setting the spurious interrupt vector register. This function must be
/// called after the `setup` function, and for each core in the system.
pub unsafe fn enable() {
    let spurious = read(Register::SpuriousInterruptVector);
    write(Register::SpuriousInterruptVector, spurious | 1 << 8);
}

/// Check if the local APIC has been initialized. This is useful to check if we can*
/// use the local APIC, especially in the early boot process.
pub fn initialized() -> bool {
    LAPIC_BASE.load(Ordering::Relaxed) != 0
}

/// Send an IPI to the given destination with the given priorit to trigger the
/// given interrupt vector.
///
/// # Safety
/// This function is unsafe because the caller must ensure that the given
/// interrupt vector is valid and can be triggered by an IPI. Furthermore, the caller needs to
/// ensure that the `setup` function has been called before, in order to set the base address of
/// the local APIC.
pub unsafe fn send_ipi(destination: IpiDestination, priority: IpiPriority, vector: u8) {
    let cmd = match destination {
        IpiDestination::Core(core) => (
            u32::from(core) << 24,
            u32::from(vector) | (priority as u32) << 8,
        ),
        IpiDestination::SelfOnly => (0, u32::from(vector) | ((priority as u32) << 8) | 1 << 18),
        IpiDestination::AllCores => (0, u32::from(vector) | ((priority as u32) << 8) | 2 << 18),
        IpiDestination::OtherCores => (0, u32::from(vector) | ((priority as u32) << 8) | 3 << 18),
    };

    write(Register::InterruptCommand1, cmd.0);
    write(Register::InterruptCommand0, cmd.1);

    // Wait for the IPI to be sent
    while read(Register::InterruptCommand0) & (1 << 12) != 0 {
        core::hint::spin_loop();
    }
}

/// Write the given value to the given register.
pub unsafe fn write(register: Register, value: u32) {
    let base = LAPIC_BASE.load(Ordering::Relaxed);
    let addr = base + register as u64;
    let ptr = addr as *mut u32;
    unsafe {
        ptr.write_volatile(value);
    }
}

/// Read the value of the given register.
pub unsafe fn read(register: Register) -> u32 {
    let base = LAPIC_BASE.load(Ordering::Relaxed);
    let addr = base + register as u64;
    let ptr = addr as *const u32;
    unsafe { ptr.read_volatile() }
}
