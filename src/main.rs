//! CH32H417 async blink demo using a minimal custom async runtime.
//!
//! No embassy, no RTOS, no alloc — just Rust's built-in async/await
//! with a simple polling executor and a software-counter delay future.
//!
//! Flashes PC2 and PC3 alternately every ~500ms.
//! HSI 8MHz internal oscillator (no external crystal needed).

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use panic_halt as _;

// ── Startup ─────────────────────────────────────────────────

global_asm!(
    r#"
.section .init, "ax"
.globl _start
_start:
    la sp, _stack_start
    jal zero, rust_main
"#
);

// ── No-op waker (single-threaded cooperative executor) ──────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

// ── CH32H417 GPIO Registers ─────────────────────────────────

const RCC_HB2PCENR: u32 = 0x4002101C;
const GPIOC_BASE: u32 = 0x40011000;
const GPIOC_CFGLR: u32 = GPIOC_BASE + 0x00;
const GPIOC_BSHR: u32 = GPIOC_BASE + 0x10;
const GPIOC_SPEED: u32 = GPIOC_BASE + 0x1C;
const PC2_SET: u32 = 1 << 2;
const PC2_RST: u32 = 1 << (16 + 2);
const PC3_SET: u32 = 1 << 3;
const PC3_RST: u32 = 1 << (16 + 3);

// ── Software-Counter Delay Future ───────────────────────────

struct Delay {
    /// Each poll decrements this. Reaches 0 → Ready.
    remaining: u32,
}

impl Delay {
    /// Approximate millisecond delay using poll counting.
    /// HSI=8MHz, ~160 executor poll iterations per ms.
    fn ms(ms: u32) -> Self {
        Self {
            remaining: ms.saturating_mul(160),
        }
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.remaining == 0 {
            Poll::Ready(())
        } else {
            self.remaining -= 1;
            Poll::Pending
        }
    }
}

// ── Async Blink Task ────────────────────────────────────────

async fn blink() {
    // GPIO init
    unsafe {
        write_volatile(
            RCC_HB2PCENR as *mut u32,
            read_volatile(RCC_HB2PCENR as *mut u32) | 0x10,
        );
        let c = GPIOC_CFGLR as *mut u32;
        write_volatile(
            c,
            (read_volatile(c) & !(0xFF << 8)) | (0x1 << 8) | (0x1 << 12),
        );
        let s = GPIOC_SPEED as *mut u32;
        write_volatile(
            s,
            (read_volatile(s) & !(0xF << 4)) | (0x3 << 4) | (0x3 << 6),
        );
    }

    loop {
        // LED1 on, LED2 off
        unsafe {
            write_volatile(GPIOC_BSHR as *mut u32, PC2_SET | PC3_RST);
        }
        Delay::ms(500).await;

        // LED1 off, LED2 on
        unsafe {
            write_volatile(GPIOC_BSHR as *mut u32, PC2_RST | PC3_SET);
        }
        Delay::ms(500).await;
    }
}

// ── Minimal Executor ────────────────────────────────────────

fn run<F: Future>(f: F) -> F::Output {
    let mut f = f;
    let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

// ── Entry ───────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    run(blink());
    loop {} // unreachable
}
