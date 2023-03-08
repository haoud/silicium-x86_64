use crate::{
    cpu::{lidt, Privilege},
    segment::{self, Selector},
};
use bitfield::{BitMut, BitRangeMut};
use core::arch::asm;

#[non_exhaustive]
#[repr(u8)]
pub enum ExceptionVector {
    DivideByZero = 0,
    Debug = 1,
    NonMaskableInterrupt = 2,
    Breakpoint = 3,
    Overflow = 4,
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    CoprocessorSegmentOverrun = 9,
    InvalidTSS = 10,
    SegmentNotPresent = 11,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    Reserved1 = 15,
    X87FloatingPoint = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SIMD = 19,
    Virtualization = 20,
    ControlProtection = 21,
    Reserved2 = 22,
    Reserved3 = 23,
    Reserved4 = 24,
    Reserved5 = 25,
    Reserved6 = 26,
    Reserved7 = 27,
    HypervisorInjection = 28,
    VmmCommunication = 29,
    Security = 30,
    Reserved8 = 31,
}

#[repr(C, align(16))]
pub struct Table {
    entries: [Descriptor; Self::SIZE],
    register: Register,
}

impl Table {
    const SIZE: usize = 256;

    /// Creates a new empty IDT. All entries are set to the MISSING descriptor by default. If a
    /// MISSING descriptor is triggered, a general protection fault is raised.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [Descriptor::MISSING; Self::SIZE],
            register: Register::null(),
        }
    }

    /// Returns the total number of entries in the IDT.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        Self::SIZE
    }

    /// Set the IDT entry at the given index to the given descriptor.
    pub fn set_descriptor(&mut self, index: u8, descriptor: Descriptor) {
        self.entries[index as usize] = descriptor;
    }

    /// Set the IDT register to point to the IDT and load it into the CPU.
    #[allow(clippy::cast_possible_truncation)]
    pub fn load(&mut self) {
        self.register.limit = (core::mem::size_of::<Descriptor>() * self.entries.len() - 1) as u16;
        self.register.base = self.entries.as_ptr() as u64;
        unsafe {
            self.register.load();
        }
    }
}

#[repr(C, packed)]
pub struct Descriptor {
    offset_low: u16,
    selector: u16,
    flags: DescriptorFlags,
    offset_middle: u16,
    offset_high: u32,
    zero: u32,
}

impl Descriptor {
    pub const MISSING: Self = Self::missing();
    /// Create a new descriptor with the default values. The default values are:
    /// - The descriptor is not marked as present
    /// - The handler address is set to 0
    /// - The descriptor flags are set to the default flags (see [`DescriptorFlags::new`])
    /// - The segment selector is set to the kernel code segment
    #[must_use]
    pub const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: segment::Selector::KERNEL_CODE64.value(),
            flags: DescriptorFlags::new(),
            offset_middle: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    /// Create a new descriptor with the default values. See [`missing`] for more details.
    #[must_use]
    pub const fn new() -> Self {
        Self::missing()
    }

    /// Set the address of the handler. The handler should be a function generated by the
    /// [`interrupt_handler`] macro, because rust functions cannot be called directly when a
    /// interrupt is triggered.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_handler_addr(&mut self, handler: u64) -> &mut Self {
        self.offset_middle = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.offset_low = handler as u16;
        self
    }

    /// Set the descriptor flags. The default is to set the present bit and to disable interrupts
    /// when the handler is invoked (see [`DescriptorFlags`] for more details)
    #[must_use]
    pub fn set_options(&mut self, flags: DescriptorFlags) -> &mut Self {
        self.flags = flags;
        self
    }

    /// Set the segment selector that will be loaded into the CS register when the handler is
    /// invoked. The default is the kernel code segment
    #[must_use]
    pub fn set_selector(&mut self, selector: Selector) -> &mut Self {
        self.selector = selector.value();
        self
    }

    /// Build the descriptor from the current state.
    #[must_use]
    pub fn build(&mut self) -> Self {
        let mut result = Self::new();
        core::mem::swap(&mut result, self);
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct DescriptorFlags(u16);

impl DescriptorFlags {
    #[must_use]
    pub const fn new() -> Self {
        Self(0x0F00)
    }

    /// Set or reset the present bit. If the present bit is not set, the CPU will raise a
    /// general protection fault when the handler is invoked.
    #[must_use]
    pub fn present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    /// Set the interrupt gate type and enable or not interrupts when the handler is invoked. If
    /// enabled is set to false (default), the IF flag is cleared when the handler is invoked.
    #[must_use]
    pub fn with_interrupts(&mut self, enabled: bool) -> &mut Self {
        self.0.set_bit(8, !enabled);
        self
    }

    /// Set the required privilege level (DPL) for invoking the handler via the `int` instruction.
    /// The default is 0 (kernel). If CPL < DPL when the handler is invoked, the CPU will raise a
    /// general protection fault. If a interrupt is triggered by the hardware, the DPL is ignored.
    /// This is useful to prevent user code from invoking privileged handlers.
    #[must_use]
    pub fn set_privilege_level(&mut self, dpl: Privilege) -> &mut Self {
        self.0.set_bit_range(15, 13, dpl as u16);
        self
    }

    /// Set the stack index for the handler. The default is 0 (no IST). The index represents the
    /// index of the stack in the TSS. The hardware will use the stack at the given index when the
    /// handler is invoked. This is useful to prevent stack overflows when the handler.
    #[must_use]
    pub fn set_stack_index(&mut self, index: u16) -> &mut Self {
        // The hardware IST index starts at 1 (0 means no IST).
        self.0.set_bit_range(3, 0, index + 1);
        self
    }

    /// Build the descriptor flags.
    #[must_use]
    pub fn build(&mut self) -> Self {
        let mut result = Self::new();
        core::mem::swap(&mut result, self);
        result
    }
}

impl Default for DescriptorFlags {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C, packed)]
pub struct Register {
    limit: u16,
    base: u64,
}

impl Register {
    /// Create a new IDT register with a null base and limit.
    #[must_use]
    pub const fn null() -> Self {
        Self { limit: 0, base: 0 }
    }

    /// Set the IDT register to point to the given IDT.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_table(&mut self, table: &Table) {
        self.limit = (core::mem::size_of::<Descriptor>() * table.entries.len() - 1) as u16;
        self.base = table as *const Table as u64;
    }

    /// Return a pointer to itself.
    #[must_use]
    pub fn pointer(&self) -> u64 {
        self as *const Self as u64
    }

    /// Load the IDT register into the CPU. This is unsafe because the caller must ensure that the
    /// IDT is valid and that the IDT register is correctly set.
    pub unsafe fn load(&self) {
        lidt(self.pointer());
    }
}

/// This macro generates an interrupt handler.
///
/// The handler is a naked function that pushes the interrupt ID and error code (if any) on the
/// stack, calls the [`interrupt_enter`] function, calls the handler function, and then calls the
/// [`interrupt_exit`] function.
/// This macro is necessary because it is not possible to call a Rust function directly when an
/// interrupt is triggered: the interrupt handler must be a naked function that does not use the
/// stack. The handler must also saved the registers that are not automatically saved by the CPU in
/// order to be able to correctly restore the context when the interrupt is finished.
///
/// # Warning
/// The handler must have the following signature:
/// ``` extern "C" fn handler(_: silicium_x86_86::cpu::State) ```
///
/// In order for this function to work properly, it is important that the CPU disables interrupts
/// when the handler is invoked (see the `with_interrupts` method of the [`DescriptorFlags`]). If
/// the interrupts are not disabled, a race condition can occur when the handler is invoked while
/// performing the `swapgs` instruction. This can cause the handler to be invoked with the wrong
/// GS register, which can lead to a crash.
/// When your handler is invoked, your are free to re-enable interrupts if you want to, as their
/// previous state will be restored when the interrupt is finished.
///
/// Failure to follow these rules will result in a undefined behavior, likely a crash.
#[macro_export]
#[cfg(feature = "int_handler")]
macro_rules! interrupt_handler {
    // Generate an interrupt handler that pushes an error code on the stack (for example, a page
    // fault)
    ($id:expr, $name:ident, $handler:ident) => {
        #[naked]
        #[no_mangle]
        pub unsafe extern "C" fn $name() {
            core::arch::asm!("
                push {id}
                call interrupt_enter
                call {handler}
                jmp interrupt_exit
                ",
                id = const $id,
                handler = sym $handler,
                options(noreturn));
        }
    };
    // Should be use when the interrupt handler does not push an error code, to keep the same
    // stack layout as the other interrupt handlers.
    ($id:expr, $name:ident, $handler:ident, $err:expr) => {
        #[naked]
        #[no_mangle]
        pub unsafe extern "C" fn $name() {
            core::arch::asm!("
                push {err}
                push {id}
                call interrupt_enter
                call {handler}
                jmp interrupt_exit
                ",
                err = const $err,
                id = const $id,
                handler = sym $handler,
                options(noreturn));
        }
    };
}

/// This macro prepare a rust interrupt handler to be called. It is used by the [`interrupt_handler`]
/// macro, and performs the following actions:
///  - Clear the direction flag (DF) in the EFLAGS register. This is required by the system V ABI.
///
///  - Swap the GS register if needed with the `swapgs` instruction. The GS register is swapped if
///    the interrupt was triggered from user mode. This is required because the GS register could be
///    used by the user code, andthe kernel use it to store TLS data.
///
///  - Save the scratch registers (RAX, RCX, RDX, RSI, RDI, R8, R9, R10, R11) on the stack.
///
///  - Save the preserved registers (RBX, RBP, R12, R13, R14, R15) on the stack.
///
///  - Save the FS register on the stack (the FS register is used to store the TLS data when
///    compiling the kernel, and I don't know how to change it to force the compiler to use the GS
///    register).
///
///  - Prepare the argument for the handler. The argument is a pointer to the stack, which contains
///   the saved registers.
///
#[naked]
#[no_mangle]
#[linkage = "weak"]
#[cfg(feature = "int_handler")]
pub unsafe extern "C" fn interrupt_enter() {
    asm!(
        "
        # Needed by the system V ABI
        cld

        # Swap gs if needed
        cmp QWORD PTR [rsp + 8 * 2], 0x08    # 0x08 is the selector for the CS kernel selector
        je 1f
        swapgs
       1:
        
        # Save scratch registers
        push r11
        push r10
        push r9
        push r8
        push rdi
        push rsi
        push rdx
        push rcx
        push rax

        # Save preserved registers
        push r15
        push r14
        push r13
        push r12
        push rbx
        push rbp

        # RDMSR for saving the FS register
        mov rax, 0xC0000100
        rdmsr
        push rdx

        # Get the kernel GS register with RDMSR
        mov rax, 0xC0000101
        rdmsr

        # Set the FS register with WRMSR
        mov rdx, rax
        mov rax, 0xC0000100
        wrmsr

        # Stack should be aligned on a 16 bytes boundary
        # Prepare the argument for the handler
        mov rdi, rsp

        # We pushed 16 registers, so the return address is at rsp + 16 * 8
        mov rax, [rsp + 16 * 8]
        jmp rax
        ",
        options(noreturn)
    );
}

/// This macro restore the context after an interrupt. It is used by the [`interrupt_handler`] macro,
/// and performs the following actions (the opposite of the [`interrupt_enter`] macro):
/// - Restore the FS register.
/// - Restore the preserved registers (RBX, RBP, R12, R13, R14, R15) from the stack.
/// - Restore the scratch registers (RAX, RCX, RDX, RSI, RDI, R8, R9, R10, R11) from the stack.
/// - Skip the error code and the interrupt ID on the stack, skip the return address used by the
///  [`interrupt_enter`] macro
/// - Restore the GS register if needed with the `swapgs` instruction (see the [`interrupt_enter`]
///  macro for more information).
/// - Perform an `iretq` instruction to restore the context.
#[naked]
#[no_mangle]
#[linkage = "weak"]
#[cfg(feature = "int_handler")]
pub unsafe extern "C" fn interrupt_exit() {
    asm!(
        "
        # Restore FS
        pop rdx
        mov rax, 0xC0000100
        wrmsr

        # Restore preserved registers
        pop rbp
        pop rbx
        pop r12
        pop r13
        pop r14
        pop r15

        # Restore scratch registers
        pop rax
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop r8
        pop r9
        pop r10
        pop r11

        # Skip error code, interrupt number and return address
        add rsp, 8 * 3

        # Swapgs if necessary
        cli                              # To avoid race condition
        cmp QWORD PTR [rsp + 8], 0x08    # 0x08 is the selector for the CS kernel selector
        je 1f
        swapgs
       1:
        iretq",
        options(noreturn)
    );
}

#[cfg(test)]
mod test {
    use core::mem::size_of;

    #[test]
    fn struct_size_checks() {
        assert_eq!(size_of::<super::Descriptor>(), 16);
        assert_eq!(size_of::<super::Register>(), 10);
    }
}
