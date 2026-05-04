#![no_std]
#![no_main]

use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

mod interrupt;
mod pac;
mod rtt;

use panic_halt as _;

// ── Startup ─────────────────────────────────────────────────

global_asm!(
    r#"
.section .init, "ax"
.globl _start
.align 2
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

    la t0, _vector_base
    ori t0, t0, 3
    csrw mtvec, t0

    li t0, 0x0F
    csrw 0x804, t0

    li t0, 0x1888
    csrw mstatus, t0

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
    .word   default_handler
    .word   SysTick1_Handler
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

// ── Default trap handler ─────────────────────────────────────

global_asm!(
    r#"
.section .trap, "ax"
.align 2
.globl default_handler
default_handler:
    j default_handler
"#
);

// ── SysTick1 interrupt handler ───────────────────────────────

interrupt_handler!(SysTick1_Handler, __rust_systick1_handler);

#[unsafe(no_mangle)]
extern "C" fn __rust_systick1_handler() {
    let isr = unsafe { read_volatile(STK0_ISR as *const u32) };
    if isr & (1 << 1) != 0 {
        unsafe { write_volatile(STK0_ISR as *mut u32, isr & !(1 << 1)); }
    }
    TICK_EXPIRED.set();
}

// ── CH32H417 Peripherals ────────────────────────────────────

const RCC_HB2PCENR: u32 = pac::RCC_BASE + pac::RCC_HB2PCENR_OFFSET; // 0x4002101C
const GPIOC_BASE: u32 = pac::GPIOC_BASE; // 0x40011000
const GPIOC_CFGLR: u32 = GPIOC_BASE + pac::GPIO_CFGLR_OFFSET; // 0x40011000
const GPIOC_BSHR: u32 = GPIOC_BASE + pac::GPIO_BSHR_OFFSET; // 0x40011010
const GPIOC_SPEED: u32 = GPIOC_BASE + pac::GPIO_SPEED_OFFSET; // 0x4001101C
const PC2_SET: u32 = 1 << 2;
const PC2_RST: u32 = 1 << (16 + 2);
const PC3_SET: u32 = 1 << 3;
const PC3_RST: u32 = 1 << (16 + 3);

const STK1_CTLR: u32 = pac::SYSTICK1_BASE + pac::STK_CTLR_OFFSET; // 0xE000F080
const STK1_CNT: u32 = pac::SYSTICK1_BASE + pac::STK_CNT_OFFSET; // 0xE000F088
const STK1_CMP: u32 = pac::SYSTICK1_BASE + pac::STK_CMP_OFFSET; // 0xE000F090
const STK0_ISR: u32 = pac::SYSTICK0_BASE + pac::STK_ISR_OFFSET; // 0xE000F004

/// HCLK frequency in Hz. CH32H417 HSI = 25MHz.
const HCLK: u32 = pac::HSI_VALUE;

// ── Tick flag (UnsafeCell avoids AtomicBool LR/SC issues) ────

struct TickFlag(UnsafeCell<bool>);
unsafe impl Sync for TickFlag {}

static TICK_EXPIRED: TickFlag = TickFlag(UnsafeCell::new(false));

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
    fn load(&self) -> bool {
        unsafe { read_volatile(self.0.get()) }
    }
    fn swap_clear(&self) -> bool {
        unsafe {
            let old = read_volatile(self.0.get());
            write_volatile(self.0.get(), false);
            old
        }
    }
}

// ── SysTick1 interrupt config ─────────────────────────────────

fn systick_interrupt_enable() {
    unsafe {
        // Enable global interrupts via CSR 0x800 (QingKe GINTR)
        core::arch::asm!("csrs 0x800, {}", in(reg) 0x88u32);
    }
}

// ── Waker ────────────────────────────────────────────────────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

// ── Hardware SysTick Delay Future ────────────────────────────

struct Delay {
    _until: u32,
}
impl Delay {
    fn ms(ms: u32) -> Self {
        let ticks = HCLK / 1000 * ms;
        TICK_EXPIRED.clear();
        unsafe {
            write_volatile(
                STK0_ISR as *mut u32,
                read_volatile(STK0_ISR as *mut u32) & !(1 << 1),
            );
            write_volatile(STK1_CNT as *mut u32, 0);
            write_volatile(STK1_CMP as *mut u32, ticks);
            write_volatile(STK1_CTLR as *mut u32, (1 << 2) | (1 << 1) | (1 << 0));
        }
        Self { _until: ticks }
    }
}
impl Future for Delay {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        // Check interrupt flag first, then fallback to ISR polling
        let isr = unsafe { read_volatile(STK0_ISR as *const u32) };
        if TICK_EXPIRED.swap_clear() || (isr & (1 << 1) != 0) {
            if isr & (1 << 1) != 0 {
                unsafe {
                    write_volatile(STK0_ISR as *mut u32, isr & !(1 << 1));
                }
            }
            unsafe {
                write_volatile(STK1_CTLR as *mut u32, 0);
            }
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
impl Drop for Delay {
    fn drop(&mut self) {
        unsafe {
            write_volatile(STK1_CTLR as *mut u32, 0);
        }
    }
}

// ── Async blink ─────────────────────────────────────────────

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

// ── Executor (with WFI sleep) ─────────────────────────────────

fn run<F: Future>(f: F) -> F::Output {
    let mut f = f;
    let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) {
            return v;
        }
        if TICK_EXPIRED.load() {
            continue;
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rtt::init();
    rtt::write_str("[BOOT] CH32H417 booted\n");
    systick_interrupt_enable();
    run(blink());
    loop {}
}
