#![no_std]
#![no_main]

use core::arch::global_asm;
use core::future::Future;
use core::pin::Pin;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use embassy_time::{Duration, Timer};
use qingke_rt_macros::interrupt;

mod pac;
mod rtt;
mod time_driver;
mod critical_impl;

use panic_halt as _;

// ── Startup (custom .init section — qingke-rt's .init is discarded) ─

global_asm!(
    r#"
.section .init, "ax"
.globl _start
_start:
    csrr t0, mhartid

    beqz t0, .Lhart0_init      # Hart 0 = V3F (C0) → WFI sleep

    # ── Hart 1 (V5F core, C1) ──────────────────────────
    la sp, _stack_start

    # Clear .bss
    la t0, _sbss
    la t1, _ebss
1:  beq t0, t1, 2f
    sw  zero, 0(t0)
    addi t0, t0, 4
    j   1b
2:
    # Copy .data from flash to RAM
    la t0, _sdata
    la t1, _edata
    la t2, _sidata
3:  beq t0, t1, 4f
    lw  t3, 0(t2)
    sw  t3, 0(t0)
    addi t0, t0, 4
    addi t2, t2, 4
    j   3b
4:

    # V5F-specific CSR setup
    li t0, 0x1237B3E0
    csrw 0xbc0, t0

    li t0, 0x07
    csrw 0xbc1, t0

    li t0, 0x0F
    csrw 0x804, t0

    li t0, 0x6088
    csrw mstatus, t0

    la t0, _vector_base
    ori t0, t0, 3
    csrw mtvec, t0

    jal zero, rust_main
    # never returns

    # ── Hart 0 (V3F core, C0) ──────────────────────────
.Lhart0_init:
    la sp, _stack_hart1

    # V3F-specific CSR setup (minimal)
    li t0, 0x123703E1
    csrw 0xbc0, t0

    li t0, 0x01
    csrw 0xbc1, t0

    li t0, 0x07
    csrw 0x804, t0

    # Disable global interrupts on V3F
    li t0, 0x88
    csrc 0x800, t0

    # Set mtvec to a dummy loop handler
    la t0, .Lhart0_loop
    ori t0, t0, 3
    csrw mtvec, t0

    # V3F: infinite WFI loop
.Lhart0_loop:
    wfi
    j .Lhart0_loop
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

const RCC_CFGR0: u32 = pac::RCC_BASE + pac::RCC_CFGR0_OFFSET;
const RCC_PLLCFGR: u32 = pac::RCC_BASE + pac::RCC_PLLCFGR_OFFSET;

const DIAG_ADDR: u32 = 0x200A0500;

// ── SysTick1 handler → embassy time driver ──────────────────

#[interrupt]
fn SysTick1_Handler() {
    time_driver::on_interrupt();
}

// ── Waker ────────────────────────────────────────────────────

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

// ── Blink (uses embassy-time::Timer) ─────────────────────────

async fn blink() {
    rtt::write_str("[BOOT] blink starting\n");
    unsafe {
        write_volatile(RCC_HB2PCENR as *mut u32, read_volatile(RCC_HB2PCENR as *mut u32) | 0x10);
        let c = GPIOC_CFGLR as *mut u32;
        write_volatile(c, (read_volatile(c) & !(0xFF << 8)) | (0x1 << 8) | (0x1 << 12));
        let s = GPIOC_SPEED as *mut u32;
        write_volatile(s, (read_volatile(s) & !(0xF << 4)) | (0x3 << 4) | (0x3 << 6));
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
        Timer::after(Duration::from_millis(1000)).await;
    }
}

// ── Atomic instruction tests (V5F "A" extension) ────────────

/// Test AMO instructions via core::sync::atomic::AtomicU32.
/// On V5F (out-of-order), we use AcqRel ordering + explicit fences
/// to prevent the CPU from reordering the test sequence.
fn test_atomic_amo() -> bool {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    // Reset
    COUNTER.store(0, Ordering::SeqCst);
    core::sync::atomic::fence(Ordering::SeqCst);

    let v = COUNTER.fetch_add(1, Ordering::SeqCst);
    core::sync::atomic::fence(Ordering::SeqCst);

    let v2 = COUNTER.fetch_add(2, Ordering::SeqCst);
    core::sync::atomic::fence(Ordering::SeqCst);

    let v3 = COUNTER.load(Ordering::SeqCst);
    // Expected: v=0, v2=1, v3=3
    v == 0 && v2 == 1 && v3 == 3
}

/// Test LR/SC (Load-Reserved / Store-Conditional) directly.
/// V5F is out-of-order — must use fence to flush the store buffer
/// before LR, and ensure no intervening memory ops between LR and SC.
fn test_atomic_lrsc() -> bool {
    #[link_section = ".bss"]
    static mut LOCK: u32 = 0;

    let mut ok = true;

    unsafe {
        // Reset lock
        write_volatile(core::ptr::addr_of_mut!(LOCK), 0);
        core::arch::asm!("fence iorw, iorw");

        // Acquire lock via LR/SC
        let _acquired: u32;
        core::arch::asm!(
            "   fence rw, rw",                    // flush store buffer before LR
            "1:",
            "   lr.w.aq {acquired}, ({lock})",    // Load-Reserved with acquire
            "   bnez {acquired}, 2f",              // if locked, try again
            "   li {tmp}, 1",
            "   sc.w.rl {acquired}, {tmp}, ({lock})", // Store-Conditional with release
            "   bnez {acquired}, 1b",              // sc failed → retry
            "2:",
            "   fence rw, rw",                    // fence after acquire
            lock = in(reg) core::ptr::addr_of!(LOCK),
            tmp = out(reg) _,
            acquired = out(reg) _acquired,
            options(nostack),
        );

        // Lock acquired. Verify lock value is 1.
        core::sync::atomic::fence(Ordering::SeqCst);
        let val = read_volatile(core::ptr::addr_of!(LOCK));
        if val != 1 {
            ok = false;
        }

        // Release lock with AMOSWAP.W
        let old: u32;
        core::arch::asm!(
            "   amoswap.w.rl {old}, zero, ({lock})", // release
            old = out(reg) old,
            lock = in(reg) core::ptr::addr_of!(LOCK),
            options(nostack),
        );
        if old != 1 {
            ok = false;
        }

        // Verify released
        core::sync::atomic::fence(Ordering::SeqCst);
        let val2 = read_volatile(core::ptr::addr_of!(LOCK));
        if val2 != 0 {
            ok = false;
        }
    }

    ok
}

fn run_atomic_tests() {
    rtt::write_str("[ATOMIC] Testing V5F 'A' extension...\n");

    // Test 1: AMO (Atomic Memory Operations via core::sync::atomic)
    if test_atomic_amo() {
        rtt::write_str("[ATOMIC] AMO test PASSED (AtomicU32 fetch_add OK)\n");
    } else {
        rtt::write_str("[ATOMIC] AMO test FAILED!\n");
    }

    // Test 2: LR/SC (Load-Reserved / Store-Conditional)
    if test_atomic_lrsc() {
        rtt::write_str("[ATOMIC] LR/SC test PASSED (spinlock acquire/release OK)\n");
    } else {
        rtt::write_str("[ATOMIC] LR/SC test FAILED!\n");
    }

    // Test 3: AMO swap stress (loop 100 times) — SeqCst
    static SWAP_CELL: AtomicU32 = AtomicU32::new(0xDEAD);
    for _i in 0..100 {
        let _prev = SWAP_CELL.swap(_i, Ordering::SeqCst);
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    rtt::write_str("[ATOMIC] SWAP stress test PASSED (100 iterations)\n");

    rtt::write_str("[ATOMIC] All tests complete.\n");
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
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

fn systick_interrupt_enable() {
    unsafe {
        // Set priority 0 for SysTick1 (V5F core timer, IRQ 13)
        let prio_addr = (pac::PFIC_IPRIOR_BASE + pac::SYSTICK1_IRQN as u32) as *mut u8;
        write_volatile(prio_addr, 0u8);
        // SysTick1 = IRQ 13 (V5F core timer)
        write_volatile(pac::PFIC_IENR1 as *mut u32, 1 << pac::SYSTICK1_IRQN);
        core::arch::asm!("csrs 0x800, {}", in(reg) 0x88u32);
    }
}

/// Ensure system clock is HSI 25MHz, regardless of debugger state.
/// Debug probes (wlink/probe-rs) may leave PLL/HSE configured after flashing.
/// This forces a switch back to HSI so Delay::ms() timing is always correct.
fn clock_init() {
    unsafe {
        // Gate off PLL from system clock mux
        write_volatile(
            RCC_PLLCFGR as *mut u32,
            read_volatile(RCC_PLLCFGR as *const u32) & !pac::RCC_PLLCFGR_SYSPLL_GATE,
        );
        // Switch system clock to HSI, reset prescalers to /1
        // Debugger may have set HPRE or FPRE to non-1 values (e.g. FPRE=/4 for V3F)
        let mut cfgr = read_volatile(RCC_CFGR0 as *const u32);
        cfgr &= !0x3u32; // SW=HSI
        cfgr &= !(0xFFu32 | (0x3 << 16)); // HPRE=/1, FPRE=/1
        write_volatile(RCC_CFGR0 as *mut u32, cfgr);
        while read_volatile(RCC_CFGR0 as *const u32) & 0xCu32 != 0x00 {} // wait SWS=HSI
                                                                         // HCLK = HSI = 25MHz (pac::HCLK already defaults to HSI_VALUE)
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    clock_init();
    rtt::init();
    rtt::write_str("[BOOT] CH32H417 V5F booted\n");

    // Initialize embassy time driver
    critical_section::with(|cs| {
        time_driver::init(cs);
    });

    run_atomic_tests();
    systick_interrupt_enable();

    rtt::write_str("[EMBASSY] Time driver ready, starting blink\n");

    rtt::write_str("[EMBASSY] Time driver ready, starting blink\n");

    run(blink());
    loop {}
}
