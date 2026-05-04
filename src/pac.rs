//! Peripheral Access Crate for CH32H417 V5F core.
//! Register addresses and bit definitions derived from C SDK (openwch/ch32h417).

#![allow(dead_code)]

// ── Base addresses ──────────────────────────────────────────────

const PERIPH_BASE: u32 = 0x4000_0000;
const HBPERIPH_BASE: u32 = PERIPH_BASE;

// ── Core peripherals (Cortex-M-like, but actually QingKe V5F PFIC) ─

pub const PFIC_BASE: u32 = 0xE000_E000;
pub const SYSTICK0_BASE: u32 = 0xE000_F000;
pub const SYSTICK1_BASE: u32 = 0xE000_F080;

// ── Peripheral base addresses ───────────────────────────────────

pub const GPIOA_BASE: u32 = HBPERIPH_BASE + 0x10800;
pub const GPIOB_BASE: u32 = HBPERIPH_BASE + 0x10C00;
pub const GPIOC_BASE: u32 = HBPERIPH_BASE + 0x11000;
pub const GPIOD_BASE: u32 = HBPERIPH_BASE + 0x11400;
pub const GPIOE_BASE: u32 = HBPERIPH_BASE + 0x11800;
pub const GPIOF_BASE: u32 = HBPERIPH_BASE + 0x11C00;

pub const RCC_BASE: u32 = HBPERIPH_BASE + 0x21000;

pub const USART1_BASE: u32 = HBPERIPH_BASE + 0x13800;
pub const USART2_BASE: u32 = HBPERIPH_BASE + 0x04400;
pub const USART3_BASE: u32 = HBPERIPH_BASE + 0x04800;

// ── GPIO register offsets ───────────────────────────────────────

pub const GPIO_CFGLR_OFFSET: u32 = 0x00; // Port config low
pub const GPIO_CFGHR_OFFSET: u32 = 0x04; // Port config high
pub const GPIO_INDR_OFFSET: u32 = 0x08; // Input data
pub const GPIO_OUTDR_OFFSET: u32 = 0x0C; // Output data
pub const GPIO_BSHR_OFFSET: u32 = 0x10; // Bit set/reset
pub const GPIO_BCR_OFFSET: u32 = 0x14; // Bit clear
pub const GPIO_LCKR_OFFSET: u32 = 0x18; // Config lock
pub const GPIO_SPEED_OFFSET: u32 = 0x1C; // Speed

// GPIO CFGLR/CFGHR mode bits (per pin, 4 bits)
// 0x0 = Analog, 0x1 = Float input, 0x4 = Output PP 10MHz, etc.
// See C SDK GPIOMode_TypeDef for full values.

// ── RCC register offsets ────────────────────────────────────────

pub const RCC_CTLR_OFFSET: u32 = 0x00;
pub const RCC_CFGR0_OFFSET: u32 = 0x04;
pub const RCC_PLLCFGR_OFFSET: u32 = 0x08;
pub const RCC_INTR_OFFSET: u32 = 0x0C;
pub const RCC_HB2PRSTR_OFFSET: u32 = 0x10;
pub const RCC_HB1PRSTR_OFFSET: u32 = 0x14;
pub const RCC_HBPCENR_OFFSET: u32 = 0x18;
pub const RCC_HB2PCENR_OFFSET: u32 = 0x1C;
pub const RCC_HB1PCENR_OFFSET: u32 = 0x20;
pub const RCC_BDCTLR_OFFSET: u32 = 0x24;
pub const RCC_RSTSCKR_OFFSET: u32 = 0x28;

// ── RCC CTLR bits ───────────────────────────────────────────────

pub const RCC_CTLR_HSION: u32 = 1 << 0;
pub const RCC_CTLR_HSIRDY: u32 = 1 << 1;
pub const RCC_CTLR_HSEON: u32 = 1 << 16;
pub const RCC_CTLR_HSERDY: u32 = 1 << 17;
pub const RCC_CTLR_HSEBYP: u32 = 1 << 18;
pub const RCC_CTLR_PLLON: u32 = 1 << 24;
pub const RCC_CTLR_PLLRDY: u32 = 1 << 25;

// ── RCC CFGR0 bits ──────────────────────────────────────────────

pub const RCC_CFGR0_SW_HSI: u32 = 0x00;
pub const RCC_CFGR0_SW_HSE: u32 = 0x01;
pub const RCC_CFGR0_SW_PLL: u32 = 0x02;
pub const RCC_CFGR0_SWS_HSI: u32 = 0x00 << 2;
pub const RCC_CFGR0_SWS_HSE: u32 = 0x01 << 2;
pub const RCC_CFGR0_SWS_PLL: u32 = 0x02 << 2;
pub const RCC_CFGR0_HPRE_DIV1: u32 = 0x00 << 4;
pub const RCC_CFGR0_FPRE_DIV1: u32 = 0x00 << 16;

// ── RCC HB2PCENR bits ───────────────────────────────────────────

pub const RCC_HB2PCENR_AFIO: u32 = 1 << 0;
pub const RCC_HB2PCENR_GPIOA: u32 = 1 << 2;
pub const RCC_HB2PCENR_GPIOB: u32 = 1 << 3;
pub const RCC_HB2PCENR_GPIOC: u32 = 1 << 4;
pub const RCC_HB2PCENR_GPIOD: u32 = 1 << 5;
pub const RCC_HB2PCENR_GPIOE: u32 = 1 << 6;
pub const RCC_HB2PCENR_GPIOF: u32 = 1 << 7;
pub const RCC_HB2PCENR_USART1: u32 = 1 << 14;

// ── RCC HB1PCENR bits ───────────────────────────────────────────

pub const RCC_HB1PCENR_USART2: u32 = 1 << 17;
pub const RCC_HB1PCENR_USART3: u32 = 1 << 18;

// ── SysTick registers (per-core) ────────────────────────────────

pub const STK_CTLR_OFFSET: u32 = 0x00;
pub const STK_ISR_OFFSET: u32 = 0x04; // only SysTick0.ISR is valid
pub const STK_CNT_OFFSET: u32 = 0x08;
pub const STK_CMP_OFFSET: u32 = 0x10;

// STK_CTLR bits
pub const STK_CTLR_STE: u32 = 1 << 0; // Counter enable
pub const STK_CTLR_STIE: u32 = 1 << 1; // Interrupt enable
pub const STK_CTLR_STCLK: u32 = 1 << 2; // 1 = HCLK, 0 = HCLK/8
pub const STK_CTLR_STRE: u32 = 1 << 3; // 1 = one-shot, 0 = auto-reload

// SysTick0 ISR flags
pub const STK0_ISR_ST0: u32 = 1 << 0; // SysTick0 flag
pub const STK0_ISR_ST1: u32 = 1 << 1; // SysTick1 flag (in SysTick0.ISR!)

pub const PFIC_SCTLR: u32 = PFIC_BASE + 0xDA0; // System Control Register
// SCTLR bits
pub const SCTLR_WFITOWFE: u32 = 1 << 3; // 0=WFI, 1=WFE
pub const SCTLR_SEVONPEND: u32 = 1 << 4; // Send Event on Pending

// ── PFIC / NVIC registers ───────────────────────────────────────

pub const PFIC_IENR0: u32 = PFIC_BASE + 0x100; // Interrupt Enable, IRQ 0-31
pub const PFIC_IENR1: u32 = PFIC_BASE + 0x104; // IRQ 32-63
pub const PFIC_IENR2: u32 = PFIC_BASE + 0x108; // IRQ 64-95
pub const PFIC_IENR3: u32 = PFIC_BASE + 0x10C; // IRQ 96-127
pub const PFIC_IENR4: u32 = PFIC_BASE + 0x110; // IRQ 128-159

pub const PFIC_IRER0: u32 = PFIC_BASE + 0x180; // Interrupt Reset Enable
pub const PFIC_IRER1: u32 = PFIC_BASE + 0x184;
pub const PFIC_IPSR0: u32 = PFIC_BASE + 0x200; // Interrupt Pending Set
pub const PFIC_IPRR0: u32 = PFIC_BASE + 0x280; // Interrupt Pending Reset

pub const PFIC_IPRIOR_BASE: u32 = PFIC_BASE + 0x400; // 256 bytes of u8 priority

// ── Interrupt numbers (from C SDK IRQn_Type) ────────────────────

pub const SYSTICK0_IRQN: u8 = 12;
pub const SYSTICK1_IRQN: u8 = 13;
pub const USART1_IRQN: u8 = 48;
pub const USART2_IRQN: u8 = 45;
pub const USART3_IRQN: u8 = 84;

// ── CSR addresses (QingKe custom CSRs) ──────────────────────────

pub const CSR_GINTR: u32 = 0x800; // Global interrupt register
pub const CSR_INTSYSCR: u32 = 0x804; // Interrupt system control (HPE, nesting)
pub const CSR_PREFETCH: u32 = 0xBC0; // Prefetch/pipe control
pub const CSR_NEST_LEVEL: u32 = 0xBC1; // Nesting depth config

// V5F-specific values
pub const V5F_PREFETCH_VAL: u32 = 0x1237_B3E0;
pub const V5F_NEST_LEVEL_VAL: u32 = 0x07; // 8-level nesting
pub const V5F_INTSYSCR_VAL: u32 = 0x0F; // HPE + nesting + 5~8 levels

// ── Clock constants ─────────────────────────────────────────────

pub const HSI_VALUE: u32 = 25_000_000;
pub const HSE_VALUE: u32 = 25_000_000;

/// Current HCLK frequency. Set by clock init.
pub static mut HCLK: u32 = HSI_VALUE;
