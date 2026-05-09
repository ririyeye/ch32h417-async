//! SysTick1-based embassy time driver for CH32H417 V5F core.
//!
//! Uses SysTick1 (at 0xE000_F080) as a 32-bit free-running counter at HCLK (25 MHz).
//! The counter is extended to 64-bit by tracking overflows in the ISR.
//! TICK_HZ = 1_000_000 (1 tick = 1 µs).

use core::cell::RefCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, Ordering};

use critical_section::CriticalSection;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time_driver::Driver;
use embassy_time_queue_utils::Queue;

use crate::pac;

/// Number of HCLK cycles per embassy-time tick (1 µs).
/// HCLK = 25 MHz, so 25 cycles per µs.
const HCLK_CYCLES_PER_TICK: u64 = 25;

embassy_time_driver::time_driver_impl!(static DRIVER: SystickDriver = SystickDriver {
    overflow_upper: AtomicU32::new(0),
    queue: Mutex::new(RefCell::new(Queue::new())),
});

pub(crate) struct SystickDriver {
    /// Upper 32 bits of the 64-bit raw counter, incremented on each SysTick1 overflow.
    overflow_upper: AtomicU32,
    /// Timer queue for scheduling wake-ups.
    queue: Mutex<CriticalSectionRawMutex, RefCell<Queue>>,
}

impl SystickDriver {
    /// Read the raw 64-bit SysTick1 counter value (in HCLK cycles).
    fn raw_count_64(&self) -> u64 {
        loop {
            let upper = self.overflow_upper.load(Ordering::Relaxed);
            let lower = read_stk1_cnt();
            // Check if an overflow occurred between reading upper and lower
            let isr = read_stk0_isr();
            if isr & pac::STK0_ISR_ST1 != 0 {
                // Overflow flag is set — the counter wrapped.
                // If lower is small, the wrap happened before we read lower.
                // If lower is large, the wrap might happen after.
                // Re-read upper to be safe.
                let new_upper = self.overflow_upper.load(Ordering::Relaxed);
                if new_upper == upper && lower > 0x8000_0000 {
                    // lower is large, wrap likely hasn't happened yet
                    return ((upper as u64) << 32) | (lower as u64);
                }
                // Else: wrap happened, use new_upper + lower
                let new_lower = read_stk1_cnt();
                return ((new_upper as u64) << 32) | (new_lower as u64);
            }
            return ((upper as u64) << 32) | (lower as u64);
        }
    }

    /// Called from SysTick1 ISR to process expired timers.
    pub(crate) fn on_interrupt(&self) {
        // Clear the SysTick1 overflow flag in STK0_ISR
        let isr = read_stk0_isr();
        write_stk0_isr(isr & !pac::STK0_ISR_ST1);

        // Increment overflow count
        self.overflow_upper.fetch_add(1, Ordering::Relaxed);

        critical_section::with(|cs| {
            self.trigger_alarm(cs);
        });
    }

    fn trigger_alarm(&self, cs: CriticalSection) {
        let mut queue = self.queue.borrow(cs).borrow_mut();
        let mut next = queue.next_expiration(self.raw_count_64());
        while !self.set_alarm(cs, next) {
            next = queue.next_expiration(self.raw_count_64());
        }
    }

    fn set_alarm(&self, _cs: CriticalSection, next_alarm_raw: u64) -> bool {
        let now = self.raw_count_64();

        if next_alarm_raw <= now {
            // Alarm already expired
            return false;
        }

        let delta = next_alarm_raw - now;
        if delta > 0xFFFF_FFFF {
            // Alarm too far in the future; set to max and let the next
            // overflow ISR re-evaluate. SysTick1 CMP is 32-bit.
            // Set CMP to now + 0xFFFF_0000 (near max)
            let cmp = now.wrapping_add(0xFFFF_0000) as u32;
            write_stk1_cmp(cmp);
            write_stk1_ctlr(0x0F); // EN | IE | HCLK | AUTORELOAD
            return true;
        }

        write_stk1_cmp(next_alarm_raw as u32);
        write_stk1_ctlr(0x0F);
        // Re-check after setting alarm
        if next_alarm_raw <= self.raw_count_64() {
            // Already passed, disarm
            write_stk1_ctlr(0x00);
            return false;
        }
        true
    }
}

impl Driver for SystickDriver {
    fn now(&self) -> u64 {
        self.raw_count_64() / HCLK_CYCLES_PER_TICK
    }

    fn schedule_wake(&self, at: u64, waker: &core::task::Waker) {
        let raw_at = at * HCLK_CYCLES_PER_TICK;
        critical_section::with(|cs| {
            let mut queue = self.queue.borrow(cs).borrow_mut();
            if queue.schedule_wake(raw_at, waker) {
                let mut next = queue.next_expiration(self.raw_count_64());
                while !self.set_alarm(cs, next) {
                    next = queue.next_expiration(self.raw_count_64());
                }
            }
        });
    }
}

/// Called from SysTick1 ISR.
pub(crate) fn on_interrupt() {
    DRIVER.on_interrupt();
}

/// Initialize the time driver hardware.
pub(crate) fn init(_cs: CriticalSection) {
    // Enable SEVONPEND: required on QingKe cores for WFI to wake from
    // interrupts that become pending while WFI is active.
    // Without this, WFI may not wake even when SysTick1 fires.
    unsafe {
        let sctlr = (pac::PFIC_BASE + 0xD10) as *mut u32;
        write_volatile(sctlr, read_volatile(sctlr) | (1 << 4)); // SEVONPEND
    }

    // Configure SysTick1 as free-running counter at HCLK
    write_stk1_cnt(0);
    write_stk1_cmp(0xFFFF_FFFF); // Max — overflow ISR will handle wrapping
    // CTLR: EN=1, IE=1, CLKSOURCE=HCLK(1), AUTORELOAD=1
    write_stk1_ctlr(0x0F);

    DRIVER.overflow_upper.store(0, Ordering::Relaxed);
}

// ── Raw register access ────────────────────────────────────────────

const STK1_CTLR: u32 = pac::SYSTICK1_BASE + pac::STK_CTLR_OFFSET;
const STK1_CNT: u32  = pac::SYSTICK1_BASE + pac::STK_CNT_OFFSET;
const STK1_CMP: u32  = pac::SYSTICK1_BASE + pac::STK_CMP_OFFSET;
const STK0_ISR: u32  = pac::SYSTICK0_BASE + pac::STK_ISR_OFFSET;

#[inline]
fn read_stk1_cnt() -> u32 {
    unsafe { core::ptr::read_volatile(STK1_CNT as *const u32) }
}

#[inline]
fn write_stk1_cnt(val: u32) {
    unsafe { core::ptr::write_volatile(STK1_CNT as *mut u32, val) }
}

#[inline]
fn write_stk1_cmp(val: u32) {
    unsafe { core::ptr::write_volatile(STK1_CMP as *mut u32, val) }
}

#[inline]
fn write_stk1_ctlr(val: u32) {
    unsafe { core::ptr::write_volatile(STK1_CTLR as *mut u32, val) }
}

#[inline]
fn read_stk0_isr() -> u32 {
    unsafe { core::ptr::read_volatile(STK0_ISR as *const u32) }
}

#[inline]
fn write_stk0_isr(val: u32) {
    unsafe { core::ptr::write_volatile(STK0_ISR as *mut u32, val) }
}
