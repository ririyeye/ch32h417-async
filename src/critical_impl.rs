//! Critical section implementation for RISC-V M-mode.
//!
//! Disables/enables machine-mode interrupts (MIE bit in mstatus).

use critical_section::{set_impl, Impl, RawRestoreState};

struct SingleCoreCriticalSection;
set_impl!(SingleCoreCriticalSection);

unsafe impl Impl for SingleCoreCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        let mut mstatus: u32;
        core::arch::asm!(
            "csrrci {}, mstatus, 0x8",
            out(reg) mstatus,
        );
        // Return whether MIE was set before we cleared it
        (mstatus & 0x8 != 0) as u8
    }

    unsafe fn release(was_mie: RawRestoreState) {
        if was_mie != 0 {
            core::arch::asm!("csrsi mstatus, 0x8");
        }
    }
}
