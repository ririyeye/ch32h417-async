#![no_std]
#![no_main]

use core::arch::global_asm;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use panic_halt as _;

// ── Startup with .bss / .data init ─────────────────────────

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

// ── CH32H417 GPIO Registers ─────────────────────────────────

const RCC_HB2PCENR: u32 = 0x4002101C;
const GPIOC_BASE:   u32 = 0x40011000;
const GPIOC_CFGLR:  u32 = GPIOC_BASE + 0x00;
const GPIOC_BSHR:   u32 = GPIOC_BASE + 0x10;
const GPIOC_SPEED:  u32 = GPIOC_BASE + 0x1C;
const PC2_SET: u32 = 1 << 2; const PC2_RST: u32 = 1 << (16 + 2);
const PC3_SET: u32 = 1 << 3; const PC3_RST: u32 = 1 << (16 + 3);

// ── No-op waker ─────────────────────────────────────────────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {}, |_| {}, |_| {},
);

// ── Software-counter Delay Future ───────────────────────────

struct Delay { remaining: u32 }
impl Delay {
    fn ms(ms: u32) -> Self { Self { remaining: ms.saturating_mul(160) } }
}
impl Future for Delay {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.remaining == 0 { Poll::Ready(()) }
        else { self.remaining -= 1; Poll::Pending }
    }
}

// ── Async blink ─────────────────────────────────────────────

async fn blink() {
    unsafe {
        write_volatile(RCC_HB2PCENR as *mut u32, read_volatile(RCC_HB2PCENR as *mut u32) | 0x10);
        let c = GPIOC_CFGLR as *mut u32;
        write_volatile(c, (read_volatile(c) & !(0xFF << 8)) | (0x1 << 8) | (0x1 << 12));
        let s = GPIOC_SPEED as *mut u32;
        write_volatile(s, (read_volatile(s) & !(0xF << 4)) | (0x3 << 4) | (0x3 << 6));
    }
    loop {
        unsafe { write_volatile(GPIOC_BSHR as *mut u32, PC2_SET | PC3_RST); }
        Delay::ms(500).await;
        unsafe { write_volatile(GPIOC_BSHR as *mut u32, PC2_RST | PC3_SET); }
        Delay::ms(500).await;
    }
}

// ── Minimal executor ────────────────────────────────────────

fn run<F: Future>(f: F) -> F::Output {
    let mut f = f;
    let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) { return v; }
    }
}

// ── Entry ───────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    run(blink());
    loop {}
}
