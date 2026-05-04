#![no_std]
#![no_main]

use core::arch::global_asm;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
mod rtt;

use panic_halt as _;

// ── Startup ─────────────────────────────────────────────────

global_asm!(r#"
.section .init, "ax"
.globl _start
_start:
    la sp, _stack_start
    la t0, _sbss
    la t1, _ebss
1:  beq t0, t1, 2f
    sw  zero, 0(t0)
    addi t0, t0, 4
    j   1b
2:  la t0, _sdata
    la t1, _edata
    la t2, _sidata
3:  beq t0, t1, 4f
    lw  t3, 0(t2)
    sw  t3, 0(t0)
    addi t0, t0, 4
    addi t2, t2, 4
    j   3b
4:  jal zero, rust_main
"#);

// ── CH32H417 Peripherals ────────────────────────────────────

const RCC_HB2PCENR: u32 = 0x4002101C;
const GPIOC_BASE:   u32 = 0x40011000;
const GPIOC_CFGLR:  u32 = GPIOC_BASE + 0x00;
const GPIOC_BSHR:   u32 = GPIOC_BASE + 0x10;
const GPIOC_SPEED:  u32 = GPIOC_BASE + 0x1C;
const PC2_SET: u32 = 1 << 2; const PC2_RST: u32 = 1 << (16 + 2);
const PC3_SET: u32 = 1 << 3; const PC3_RST: u32 = 1 << (16 + 3);

// SysTick1 (V5F core timer) at 0xE000F080
// SysTick0.ISR at 0xE000F004 bit1 = SysTick1 compare flag
const STK1_CTLR: u32 = 0xE000F080; // control (bit0=enable, bit2=HCLK src)
const STK1_CNT:  u32 = 0xE000F088; // counter
const STK1_CMP:  u32 = 0xE000F090; // compare
const STK0_ISR:  u32 = 0xE000F004; // interrupt status (bit1=STK1)

/// HCLK frequency in Hz. CH32H417 HSI = 25MHz.
const HCLK: u32 = 25_000_000;

// ── Waker ────────────────────────────────────────────────────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {}, |_| {}, |_| {},
);

// ── Hardware SysTick Delay Future ────────────────────────────

struct Delay { _until: u32 }
impl Delay {
    fn ms(ms: u32) -> Self {
        let ticks = HCLK / 1000 * ms;
        unsafe {
            // Clear previous flag
            write_volatile(STK0_ISR as *mut u32, read_volatile(STK0_ISR as *mut u32) & !(1 << 1));
            // Reset counter, set compare, enable with HCLK source
            write_volatile(STK1_CNT as *mut u32, 0);
            write_volatile(STK1_CMP as *mut u32, ticks);
            write_volatile(STK1_CTLR as *mut u32, (1 << 2) | (1 << 0));
        }
        Self { _until: ticks }
    }
}
impl Future for Delay {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let isr = unsafe { read_volatile(STK0_ISR as *const u32) };
        if isr & (1 << 1) != 0 {
            unsafe { write_volatile(STK1_CTLR as *mut u32, 0); } // disable timer
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
impl Drop for Delay {
    fn drop(&mut self) {
        unsafe { write_volatile(STK1_CTLR as *mut u32, 0); }
    }
}

// ── Async blink ─────────────────────────────────────────────

async fn blink() {
    rtt::write_str("[BOOT] blink starting\n");
    unsafe {
        write_volatile(RCC_HB2PCENR as *mut u32, read_volatile(RCC_HB2PCENR as *mut u32) | 0x10);
        let c = GPIOC_CFGLR as *mut u32;
        write_volatile(c, (read_volatile(c) & !(0xFF << 8)) | (0x1 << 8) | (0x1 << 12));
        let s = GPIOC_SPEED as *mut u32;
        write_volatile(s, (read_volatile(s) & !(0xF << 4)) | (0x3 << 4) | (0x3 << 6));
    }
    const DIAG_ADDR: u32 = 0x200A0500;

    let mut tick: u32 = 0;
    loop {
        tick += 1;
        unsafe {
            write_volatile(DIAG_ADDR as *mut u32, tick);
            if tick & 1 != 0 {
                write_volatile(GPIOC_BSHR as *mut u32, PC2_SET | PC3_RST);
                rtt::write_str("[LED] on\n");
            } else {
                write_volatile(GPIOC_BSHR as *mut u32, PC2_RST | PC3_SET);
                rtt::write_str("[LED] off\n");
            }
        }
        Delay::ms(1000).await;
    }
}

// ── Executor ─────────────────────────────────────────────────

fn run<F: Future>(f: F) -> F::Output {
    let mut f = f;
    let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) { return v; }
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rtt::init();
    rtt::write_str("[BOOT] CH32H417 booted\n");
    run(blink());
    loop {}
}
