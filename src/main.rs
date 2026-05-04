#![no_std]
#![no_main]

use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

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
const GPIOC_BSHR: u32 = GPIOC_BASE + pac::GPIO_BSHR_OFFSET;
const GPIOC_SPEED: u32 = GPIOC_BASE + pac::GPIO_SPEED_OFFSET;
const PC2_SET: u32 = 1 << 2;
const PC2_RST: u32 = 1 << (16 + 2);
const PC3_SET: u32 = 1 << 3;
const PC3_RST: u32 = 1 << (16 + 3);

// SysTick0 (V3F core timer)
const STK_CTLR: u32 = pac::SYSTICK0_BASE + pac::STK_CTLR_OFFSET;
const STK_CNT: u32 = pac::SYSTICK0_BASE + pac::STK_CNT_OFFSET;
const STK_CMP: u32 = pac::SYSTICK0_BASE + pac::STK_CMP_OFFSET;
const STK_ISR: u32 = pac::SYSTICK0_BASE + pac::STK_ISR_OFFSET;

const HCLK: u32 = pac::HSI_VALUE;

const DIAG_ADDR: u32 = 0x200A0500;

// ── Tick flag ────────────────────────────────────────────────

struct TickFlag(UnsafeCell<bool>);
unsafe impl Sync for TickFlag {}

static TICK_EXPIRED: TickFlag = TickFlag(UnsafeCell::new(false));

impl TickFlag {
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

// ── SysTick0 handler (full software save, no HPE dependency) ─

global_asm!(
    r#"
.section .text, "ax"
.globl SysTick0_Handler
SysTick0_Handler:
    addi sp, sp, -68
    sw   ra,  0(sp)
    sw   t0,  4(sp)
    sw   t1,  8(sp)
    sw   t2, 12(sp)
    sw   t3, 16(sp)
    sw   t4, 20(sp)
    sw   t5, 24(sp)
    sw   t6, 28(sp)
    sw   a0, 32(sp)
    sw   a1, 36(sp)
    sw   a2, 40(sp)
    sw   a3, 44(sp)
    sw   a4, 48(sp)
    sw   a5, 52(sp)
    sw   a6, 56(sp)
    sw   a7, 60(sp)

    lui  t0, 0xe000f
    lw   t1, 4(t0)
    andi t2, t1, 1
    beqz t2, 1f
    andi t1, t1, -2
    sw   t1, 4(t0)
1:
    lui  t0, 0x200a0
    li   t1, 1
    sb   t1, 0x430(t0)

    lw   ra,  0(sp)
    lw   t0,  4(sp)
    lw   t1,  8(sp)
    lw   t2, 12(sp)
    lw   t3, 16(sp)
    lw   t4, 20(sp)
    lw   t5, 24(sp)
    lw   t6, 28(sp)
    lw   a0, 32(sp)
    lw   a1, 36(sp)
    lw   a2, 40(sp)
    lw   a3, 44(sp)
    lw   a4, 48(sp)
    lw   a5, 52(sp)
    lw   a6, 56(sp)
    lw   a7, 60(sp)
    addi sp, sp, 68
    mret
"#
);

// ── Waker ────────────────────────────────────────────────────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

// ── Delay ────────────────────────────────────────────────────

struct Delay {
    _until: u32,
}
impl Delay {
    fn ms(ms: u32) -> Self {
        let ticks = HCLK / 1000 * ms;
        TICK_EXPIRED.clear();
        unsafe {
            write_volatile(
                STK_ISR as *mut u32,
                read_volatile(STK_ISR as *mut u32) & !(1 << 0),
            );
            write_volatile(STK_CNT as *mut u32, 0);
            write_volatile(STK_CMP as *mut u32, ticks);
            write_volatile(STK_CTLR as *mut u32, (1 << 2) | (1 << 0)); // STIE=0 for now
        }
        Self { _until: ticks }
    }
}
impl Future for Delay {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let isr = unsafe { read_volatile(STK_ISR as *const u32) };
        if TICK_EXPIRED.swap_clear() || (isr & (1 << 0) != 0) {
            if isr & (1 << 0) != 0 {
                unsafe {
                    write_volatile(STK_ISR as *mut u32, isr & !(1 << 0));
                }
            }
            unsafe {
                write_volatile(STK_CTLR as *mut u32, 0);
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
            write_volatile(STK_CTLR as *mut u32, 0);
        }
    }
}

// ── Blink ────────────────────────────────────────────────────

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
        if let Poll::Ready(v) = pinned.as_mut().poll(&mut cx) {
            return v;
        }
        // Busy-poll: interrupt just sets the flag, we still spin
        if TICK_EXPIRED.load() {
            continue;
        }
    }
}

#[allow(dead_code)]
fn systick_interrupt_enable() {
    unsafe {
        // Set priority 0 for SysTick0 (like C SDK: NVIC_SetPriority(SysTick0_IRQn, 0))
        let prio_addr = (pac::PFIC_IPRIOR_BASE + pac::SYSTICK0_IRQN as u32) as *mut u8;
        write_volatile(prio_addr, 0u8);
        // SysTick0 = IRQ 12 (V3F core timer)
        write_volatile(pac::PFIC_IENR0 as *mut u32, 1 << pac::SYSTICK0_IRQN);
        core::arch::asm!("csrs 0x800, {}", in(reg) 0x88u32);
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rtt::init();
    rtt::write_str("[BOOT] CH32H417 V3F booted\n");
    // systick_interrupt_enable(); // V3F interrupt WIP
    run(blink());
    loop {}
}
