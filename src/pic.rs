use core::sync::atomic::{AtomicU8, Ordering};

use crate::io::Port;

static MASTER_PIC_CMD: Port<u8> = unsafe { Port::new(0x20) };
static MASTER_PIC_DATA: Port<u8> = unsafe { Port::new(0x21) };
static SLAVE_PIC_CMD: Port<u8> = unsafe { Port::new(0xA0) };
static SLAVE_PIC_DATA: Port<u8> = unsafe { Port::new(0xA1) };
static IRQ_BASE: AtomicU8 = AtomicU8::new(0);

/// Remap the PICs to the given base IRQs. The master PIC will use IRQs [base, base + 7] and the
/// slave PIC will use IRQs [base + 8, base + 15]. After remapping, all interrupts are unmasked,
/// but no interrupts will occur until the interrupts are enabled with the `sti` instruction.
///
/// # Safety
/// This function is unsafe because it writes to the PICs with I/O ports, which can cause undefined
/// behavior if the PICs do not exist or are not in the expected state.
pub unsafe fn remap(base: u8) {
    IRQ_BASE.store(base, Ordering::Relaxed);

    // ECW1: Cascade mode, ICW4 needed
    MASTER_PIC_CMD.write_and_pause(0x11);
    SLAVE_PIC_CMD.write_and_pause(0x11);

    // ICW2: Write the base IRQs for the PICs
    MASTER_PIC_DATA.write_and_pause(base);
    SLAVE_PIC_DATA.write_and_pause(base + 8);

    // ICW3: Connect the PICs to each other
    MASTER_PIC_DATA.write_and_pause(4); // The slave PIC is connected to IRQ4 on the master PIC
    SLAVE_PIC_DATA.write_and_pause(2); // The master PIC is connected to IRQ2 on the slave PIC

    // ICW4: Request 8086 mode
    MASTER_PIC_DATA.write_and_pause(0x01);
    SLAVE_PIC_DATA.write_and_pause(0x01);

    // OCW1: Enable all interrupts
    unmask_all();
}

/// Check if the given IRQ number is in the range of the PICs. This is useful for checking if an
/// interrupt handler should send an EOI to the PICs.
pub fn concerned(irq: u8) -> bool {
    let base = IRQ_BASE.load(Ordering::Relaxed);
    irq >= base && irq < base + 16
}

/// Send an end-of-interrupt (EOI) to the PICs. This must be called after an interrupt handler
/// finishes executing. If the IRQ number is not in the range of the PICs, this function does
/// nothing.
///
/// # Safety
/// This function is unsafe because it writes to the PICs with I/O ports, which can cause undefined
/// behavior if the PICs do not exist or are not in the expected state, or if it is used incorrectly.
pub unsafe fn send_eoi(irq: u8) {
    if concerned(irq) {
        if irq - IRQ_BASE.load(Ordering::Relaxed) >= 8 {
            SLAVE_PIC_CMD.write_and_pause(0x20);
        }
        MASTER_PIC_CMD.write_and_pause(0x20);
    }
}

/// Unmask all interrupts on the PICs. This is the default state after remapping the PICs.
///
/// # Safety
/// This function is unsafe because it writes to the PICs with I/O ports, which can cause undefined
/// behavior if the PICs do not exist or are not in the expected state.
pub unsafe fn unmask_all() {
    MASTER_PIC_DATA.write_and_pause(0x00);
    SLAVE_PIC_DATA.write_and_pause(0x00);
}

/// Mask all interrupts on the PICs. An interrupt masked by the PICs will never occur and will not
/// be sent to the CPU (lost forever).
///
/// # Safety
/// This function is unsafe because it writes to the PICs with I/O ports, which can cause undefined
/// behavior if the PICs do not exist or are not in the expected state.
pub unsafe fn mask_all() {
    MASTER_PIC_DATA.write_and_pause(0xFF);
    SLAVE_PIC_DATA.write_and_pause(0xFF);
}
