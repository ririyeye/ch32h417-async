#![no_std]
#![no_main]

use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, Waker};

use qingke_rt_macros::interrupt;

mod pac;
mod rtt;

use panic_halt as _;

// ── Startup (custom .init section — qingke-rt's .init is discarded) ─

global_asm!(
    r#"
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
4:

    li t0, 0x123703e1
    csrw 0xbc0, t0

    li t0, 0x01
    csrw 0xbc1, t0

    li t0, 0x07
    csrw 0x804, t0

    li t0, 0x6088
    csrw mstatus, t0

    la t0, _vector_base
    ori t0, t0, 3
    csrw mtvec, t0

    jal zero, rust_main
"#
);

// ── Vector table ─────────────────────────────────────────────

global_asm!(
    r#"
.section .vector, "ax"
.align 1
.globl _vector_base
.option norvc
_vector_base:
    .word   _start
    .word   0
    .word   default_handler
    .word   default_handler
    .word   0
    .word   default_handler
    .word   0
    .word   0
    .word   default_handler
    .word   default_handler
    .word   0
    .word   0
    .word   SysTick0_Handler
    .word   default_handler
    .word   default_handler
    .word   0
    .word   default_handler
    .word   default_handler
    .word   default_handler
    .word   default_handler
    .word   0
    .word   0
    .word   0
    .word   0
    .word   0
    .word   0
    .word   0
    .word   0
    .word   default_handler
    .word   0
    .word   0
    .word   0
.option rvc
"#
);

// ── Default handler ──────────────────────────────────────────

global_asm!(
    r#"
.section .trap, "ax"
.align 2
.globl default_handler
default_handler:
    j default_handler
"#
);

// ── CH32H417 Peripherals ────────────────────────────────────

const RCC_HB2PCENR: u32 = pac::RCC_BASE + pac::RCC_HB2PCENR_OFFSET;
const GPIOC_BASE: u32 = pac::GPIOC_BASE;
const GPIOC_CFGLR: u32 = GPIOC_BASE + pac::GPIO_CFGLR_OFFSET;
const GPIOC_OUTDR: u32 = GPIOC_BASE + pac::GPIO_OUTDR_OFFSET;
const GPIOC_SPEED: u32 = GPIOC_BASE + pac::GPIO_SPEED_OFFSET;
const PC2_OUT: u32 = 1 << 2;
const PC3_OUT: u32 = 1 << 3;

// SysTick0 (V3F core timer)
const STK_CTLR: u32 = pac::SYSTICK0_BASE + pac::STK_CTLR_OFFSET;
const STK_CNT: u32 = pac::SYSTICK0_BASE + pac::STK_CNT_OFFSET;
const STK_CMP: u32 = pac::SYSTICK0_BASE + pac::STK_CMP_OFFSET;
const STK_ISR: u32 = pac::SYSTICK0_BASE + pac::STK_ISR_OFFSET;

const RCC_CFGR0: u32 = pac::RCC_BASE + pac::RCC_CFGR0_OFFSET;
const RCC_PLLCFGR: u32 = pac::RCC_BASE + pac::RCC_PLLCFGR_OFFSET;

const DIAG_ADDR: u32 = 0x200A0500;

// ── Tick flag + waker (ISR ↔ Delay future) ──────────────

struct TickFlag(UnsafeCell<bool>);
unsafe impl Sync for TickFlag {}

static TICK_EXPIRED: TickFlag = TickFlag(UnsafeCell::new(false));

struct DelayWaker(UnsafeCell<Option<Waker>>);
unsafe impl Sync for DelayWaker {}
static DELAY_WAKER: DelayWaker = DelayWaker(UnsafeCell::new(None));

impl TickFlag {
    fn set(&self) {
        unsafe {
            write_volatile(self.0.get(), true);
        }
    }
    fn clear(&self) {
        unsafe {
            write_volatile(self.0.get(), false);
        }
    }
    fn swap_clear(&self) -> bool {
        unsafe {
            let old = read_volatile(self.0.get());
            write_volatile(self.0.get(), false);
            old
        }
    }
}

// ── SysTick0 handler ─────────────────────────────────────

#[interrupt]
fn SysTick0_Handler() {
    let isr = unsafe { read_volatile(STK_ISR as *const u32) };
    if isr & (1 << 0) != 0 {
        unsafe {
            write_volatile(STK_ISR as *mut u32, isr & !(1 << 0));
        }
    }
    TICK_EXPIRED.set();
    // Wake the task that's waiting on Delay
    unsafe {
        if let Some(waker) = (*DELAY_WAKER.0.get()).as_ref() {
            waker.wake_by_ref();
        }
    }
}

// ── Delay (embassy-compatible waker chain) ───────────────

struct Delay {
    ticks: u32,
    started: bool,
}

impl Delay {
    fn ms(ms: u32) -> Self {
        let hclk = unsafe { pac::HCLK };
        Self {
            ticks: hclk / 1000 * ms,
            started: false,
        }
    }
}

impl Future for Delay {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if !self.started {
            self.started = true;
            // Register waker so ISR can wake us
            unsafe {
                *DELAY_WAKER.0.get() = Some(cx.waker().clone());
            }
            TICK_EXPIRED.clear();
            unsafe {
                write_volatile(
                    STK_ISR as *mut u32,
                    read_volatile(STK_ISR as *const u32) & !(1 << 0),
                );
                write_volatile(STK_CNT as *mut u32, 0);
                write_volatile(STK_CMP as *mut u32, self.ticks);
                write_volatile(STK_CTLR as *mut u32, (1 << 2) | (1 << 1) | (1 << 0));
            }
            Poll::Pending
        } else if TICK_EXPIRED.swap_clear() {
            unsafe {
                write_volatile(STK_CTLR as *mut u32, 0);
            }
            Poll::Ready(())
        } else {
            unsafe {
                *DELAY_WAKER.0.get() = Some(cx.waker().clone());
            }
            Poll::Pending
        }
    }
}

impl Drop for Delay {
    fn drop(&mut self) {
        unsafe {
            write_volatile(STK_CTLR as *mut u32, 0);
        }
    }
}

// ── Blink ────────────────────────────────────────────────

async fn blink() {
    rtt::write_str("[BOOT] blink starting\n");
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

    let mut tick: u32 = 0;
    loop {
        tick += 1;
        unsafe {
            write_volatile(DIAG_ADDR as *mut u32, tick);
            if tick & 1 != 0 {
                write_volatile(GPIOC_OUTDR as *mut u32, PC2_OUT);
                rtt::write_str("[LED] on\n");
            } else {
                write_volatile(GPIOC_OUTDR as *mut u32, PC3_OUT);
                rtt::write_str("[LED] off\n");
            }
        }
        Delay::ms(1000).await;
    }
}

// ── Executor ─────────────────────────────────────────────
//
//  Uses a proper waker chain: Delay stores cx.waker() → ISR calls
//  waker.wake_by_ref() → task re-polled. This matches how embassy's
//  waker works, just without the multi-task run-queue.

fn run<F: Future>(f: F) -> F::Output {
    let mut f = f;
    let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        // Build a real waker that stores itself in DELAY_WAKER
        // (Delay::poll will overwrite it with cx.waker())
        let waker = unsafe {
            Waker::from_raw(core::task::RawWaker::new(
                core::ptr::null(),
                &core::task::RawWakerVTable::new(
                    |_| core::task::RawWaker::new(core::ptr::null(), &VTABLE),
                    |_| {},
                    |_| {},
                    |_| {},
                ),
            ))
        };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) {
            return v;
        }
        // WFI — but first check if tick already expired
        // (ISR may have fired between poll and check)
        if TICK_EXPIRED.swap_clear() {
            continue;
        }
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

static VTABLE: core::task::RawWakerVTable = core::task::RawWakerVTable::new(
    |_| core::task::RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

// ── Init ─────────────────────────────────────────────────

fn systick_interrupt_enable() {
    unsafe {
        let prio_addr = (pac::PFIC_IPRIOR_BASE + pac::SYSTICK0_IRQN as u32) as *mut u8;
        write_volatile(prio_addr, 0u8);
        write_volatile(pac::PFIC_IENR1 as *mut u32, 1 << pac::SYSTICK0_IRQN);
        core::arch::asm!("csrs 0x800, {}", in(reg) 0x88u32);
    }
}

fn clock_init() {
    unsafe {
        write_volatile(
            RCC_PLLCFGR as *mut u32,
            read_volatile(RCC_PLLCFGR as *const u32) & !pac::RCC_PLLCFGR_SYSPLL_GATE,
        );
        let mut cfgr = read_volatile(RCC_CFGR0 as *const u32);
        cfgr &= !0x3u32;
        cfgr &= !(0xFFu32 | (0x3 << 16));
        write_volatile(RCC_CFGR0 as *mut u32, cfgr);
        while read_volatile(RCC_CFGR0 as *const u32) & 0xCu32 != 0x00 {}
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    clock_init();
    rtt::init();
    rtt::write_str("[BOOT] CH32H417 V3F booted\n");
    systick_interrupt_enable();
    run(blink());
    loop {}
}
