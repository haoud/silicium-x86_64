use core::arch::asm;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct State {
    // Preserved registers
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Scratch registers
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,

    // Used to return from "interrupt_enter"
    address: u64,

    // Error code (if any) and interrupt number
    pub number: u64,
    pub code: u64,

    // Pushed by the CPU automatically
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rax: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            address: 0,
            number: 0,
            code: 0,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        }
    }
}

pub enum Privilege {
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl Privilege {
    pub const KERNEL: Self = Self::Ring0;
    pub const USER: Self = Self::Ring3;
}

/// Halts definitely the current CPU.
///
/// # Warning
/// This function only halts the current CPU and does not stop other CPUs.
#[inline]
pub fn freeze() -> ! {
    unsafe {
        loop {
            cli();
            hlt();
        }
    }
}

/// Disables interrupts on the current CPU. If an interrupt occurs while interrupts are disabled, it
/// will be queued and executed when interrupts are re-enabled (for example, with [`sti`])
#[inline]
pub fn cli() {
    // SAFETY: Disabling interrupts should not cause any undefined behavior
    unsafe {
        asm!("cli");
    }
}

/// Enables interrupts on the current CPU. If an interrupt was queued while interrupts were disabled,
/// it will be executed after this function returns.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the IDT or an interrupt
/// handler is not properly written.
#[inline]
pub unsafe fn sti() {
    asm!("sti");
}

/// Stop the current CPU core until the next interrupt occurs.
///
/// # Safety
/// This function is unsafe because it can cause unexpected behavior if interrupts are not enabled
/// when this function is called.
#[inline]
pub unsafe fn hlt() {
    asm!("hlt");
}

/// Load the given GDT register into the CPU. The parameter is a pointer to the
/// GDT register.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given
/// gdtr is not a valid GDT register.
#[inline]
pub unsafe fn lgdt(gdtr: u64) {
    asm!("lgdt [{}]", in(reg) gdtr, options(readonly, nostack, preserves_flags));
}

/// Load the given IDT register into the CPU. The parameter is a pointer to the
/// IDT register.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given
/// idtr is not a valid IDT register.
#[inline]
pub unsafe fn lidt(idtr: u64) {
    asm!("lidt [{}]", in(reg) idtr, options(readonly, nostack, preserves_flags));
}

/// Load a new task state segment (TSS) into the CPU. The parameter is the selector of the TSS.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given selector is not a
/// valid TSS selector, if the TSS is not loaded or not properly configured or if the GDT is not
/// loaded or not properly configured.
#[inline]
pub unsafe fn ltr(selector: u16) {
    asm!("ltr ax", in("ax") selector, options(readonly, nostack, preserves_flags));
}

/// Invalidate the TLB entry for the given virtual address.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if not correctly used.
pub unsafe fn invlpg(address: u64) {
    asm!("invlpg [{}]", in(reg) address, options(readonly, nostack, preserves_flags));
}

/// Save the current CPU state into `from` and load the state from `to`.
/// 
/// When the saved state is restored, the CPU will return to the instruction after the call to
/// `switch`, like if it had been a normal function call.
/// 
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given states are not
/// properly initialized or saved. This function only touches general purpose registers, the stack
/// pointer, the instruction pointer and the rflags. It does not touch any segment registers or
/// any other registers.
pub unsafe fn switch(from: &mut State, to: &State) {
    asm!(
        "mov rax, rsp",     // Save the current stack pointer
        "mov rsp, [{}]",    // Load the new stack pointer (from)

        /* Push the registers (usually pushed by the CPU during an interrupt) */
        "push 0",           // Null SS selector (because we are in kernel mode)
        "push rax",         // Push the saved stack pointer
        "pushfq",           // Push the current flags
        "push 0x08",        // Push the kernel code segment selector
        "push 1f",          // Push the return address

        /* Push fake error code, interrupt number and skip address */
        "push 0",
        "push 0",
        "push 0",

        /* Save scratch registers */
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rdx",
        "push rcx",
        "push rax",

        /* Save preserved registers */
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push rbx",
        "push rbp",

        "mov rsp, [{}]",    // Load the new stack pointer (to)

        /* Restore preserved registers */
        "pop rbp",
        "pop rbx",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        /* Restore scratch registers */
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",

        "add rsp, 24",      // Skip error code, number and fake return address
        "iretq",            // Return to the new state

        /* When a state is restored, it return to this address */
        "1:",
        in(reg) from,
        in(reg) to,
        out("rax") _,   
        options(nostack, preserves_flags)
    );
}

pub mod cr0 {
    use core::arch::asm;

    use bitflags::bitflags;

    bitflags! {
        pub struct Flags: u64 {
            /// Protected mode
            const PE = 1 << 0;

            /// Monitor co-processor
            const MP = 1 << 1;

            /// Emulation
            const EM = 1 << 2;

            /// Task switched
            const TS = 1 << 3;

            /// Extension type
            const ET = 1 << 4;

            /// Numeric error
            const NE = 1 << 5;

            /// Write protect
            const WP = 1 << 16;

            /// Alignment mask
            const AM = 1 << 18;

            /// Not write-through
            const NW = 1 << 29;

            /// Cache disable
            const CD = 1 << 30;

            /// Paging
            const PG = 1 << 31;
        }
    }

    /// Read the current value of the control register 0 (CR0).
    #[must_use]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!("mov {}, cr0", out(reg) value, options(nostack, preserves_flags));
        }
        value
    }

    /// Write the given value to the control register 0 (CR0).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior if the address is not a valid
    /// physical address of a valid pml4 table, or if the address is not aligned on a 4KiB boundary.
    pub unsafe fn write(address: u64) {
        asm!("mov cr0, {}", in(reg) address, options(nostack, preserves_flags));
    }

    /// Set the given flags in the control register 0 (CR0).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior (depending on the flags
    /// set). If a flag set is not supported by the CPU, it will cause a general protection fault.
    pub unsafe fn set(flags: Flags) {
        write(read() | flags.bits());
    }

    /// Clear the given flags in the control register 0 (CR0).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior (depending on the flags
    /// cleared).
    pub unsafe fn clear(flags: Flags) {
        write(read() & !flags.bits());
    }
}

pub mod cr2 {
    use core::arch::asm;

    /// Read the current value of the control register 2 (CR0).
    #[must_use]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!("mov {}, cr2", out(reg) value, options(nostack, preserves_flags));
        }
        value
    }

    /// Write the given value to the control register 2 (CR0).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior.
    pub unsafe fn write(address: u64) {
        asm!("mov cr2, {}", in(reg) address, options(nostack, preserves_flags));
    }
}

pub mod cr3 {
    use core::arch::asm;

    /// Read the current value of the control register 3 (CR0).
    #[must_use]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack, preserves_flags));
        }
        value
    }

    /// Write the given value to the control register 3 (CR3).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior if the address is not a valid
    /// physical address of a valid pml4 table, or if the address is not aligned on a 4KiB boundary.
    pub unsafe fn write(address: u64) {
        asm!("mov cr3, {}", in(reg) address, options(nostack, preserves_flags));
    }

    /// Reload the current value of the control register 3 (CR3) with the same value that is already
    /// stored in the register.
    /// This is useful to flush the TLB (but the pages marked as global are not flushed).
    pub unsafe fn reload() {
        write(read());
    }
}

pub mod cr4 {
    use core::arch::asm;

    use bitflags::bitflags;

    bitflags! {
        pub struct Flags: u64 {
            /// Virtual-8086 mode extensions
            const VME = 1 << 0;

            /// Protected-mode virtual interrupts
            const PVI = 1 << 1;

            /// Time stamp disabled for user mode. If set, the RDTSC instruction is not available
            /// to user mode, only to privileged mode (ring 0)
            const TSD = 1 << 2;

            /// Debugging extensions
            const DE = 1 << 3;

            /// Page size extensions
            const PSE = 1 << 4;

            /// Physical address extension
            const PAE = 1 << 5;

            /// Machine check enable
            const MCE = 1 << 6;

            /// Page global enable
            const PGE = 1 << 7;

            /// Performance monitoring counter enable
            const PCE = 1 << 8;

            /// Operating system support for FXSAVE and FXRSTOR instructions
            const OSFXSR = 1 << 9;

            /// Operating system support for unmasked SIMD floating-point exceptions
            const OSXMMEXCPT = 1 << 10;

            /// User-mode instruction prevention
            const UMIP = 1 << 11;

            /// Virtual machine extensions enable
            const VMXE = 1 << 13;

            /// Safer mode extensions enable
            const SMXE = 1 << 14;

            /// Enable `rdfsbase`, `rdgsbase`, `wrfsbase`, and `wrgsbase` instructions
            const FSGSBASE = 1 << 16;

            /// PCID enable
            const PCIDE = 1 << 17;

            /// XSAVE and Processor Extended States
            const OSXSAVE = 1 << 18;

            /// Supervisor Mode Execution Protection
            const SMEP = 1 << 20;

            /// Supervisor Mode Access Prevention
            const SMAP = 1 << 21;

            /// Protection Keys for User Pages
            const PKE = 1 << 22;

            /// Control-flow Enforcement Technology
            const CET = 1 << 23;

            /// Protection Keys for Supervisor Pages
            const PKS = 1 << 24;
        }
    }

    /// Read the current value of the control register 4 (CR4).
    #[must_use]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!("mov {}, cr4", out(reg) value, options(nostack, preserves_flags));
        }
        value
    }

    /// Write the given value to the control register 4 (CR4).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior if the address is not a valid
    /// physical address of a valid pml4 table, or if the address is not aligned on a 4KiB boundary.
    pub unsafe fn write(address: u64) {
        asm!("mov cr4, {}", in(reg) address, options(nostack, preserves_flags));
    }

    /// Set the given flags in the control register 4 (CR4).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior (depending on the flags
    /// set). If a flag set is not supported by the CPU, it will cause a general protection fault.
    pub unsafe fn set(flags: Flags) {
        write(read() | flags.bits());
    }

    /// Clear the given flags in the control register 4 (CR4).
    ///
    /// # Safety
    /// This function is unsafe because it can cause undefined behavior (depending on the flags
    /// cleared).
    pub unsafe fn clear(flags: Flags) {
        write(read() & !flags.bits());
    }
}

pub mod msr {
    use core::arch::asm;

    pub enum Register {
        Efer = 0xC0000080,
        Star = 0xC0000081,
        Lstar = 0xC0000082,
        Cstar = 0xC0000083,
        Fmask = 0xC0000084,
        FsBase = 0xC0000100,
        GsBase = 0xC0000101,
        KernelGsBase = 0xC0000102,
    }

    pub unsafe fn write(msr: Register, value: u64) {
        asm!("wrmsr", in("ecx") msr as u32, in("eax") (value as u32), in("edx") (value >> 32));
    }

    pub unsafe fn read(msr: Register) -> u64 {
        let low: u32;
        let high: u32;
        asm!("rdmsr", in("ecx") msr as u32, out("eax") low, out("edx") high);
        (high as u64) << 32 | (low as u64)
    }
}
