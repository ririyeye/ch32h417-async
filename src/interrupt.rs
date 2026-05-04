//! Minimal interrupt handler shim — replicates ch32-hal / qingke-rt pattern.
//!
//! The `interrupt!` macro generates a WCH-Interrupt-fast ABI-compatible
//! assembly trampoline that saves `ra`, calls the Rust handler, restores `ra`,
//! and returns via `mret`. Hardware Prologue/Epilogue (HPE) must be enabled
//! via CSR 0x804 in startup for this to be safe — HPE saves all other
//! caller-saved registers (x5-x31).

#[macro_export]
macro_rules! interrupt_handler {
    ($handler_name:ident, $rust_fn:ident) => {
        ::core::arch::global_asm!(
            ::core::concat!(
                ".section .trap, \"ax\"\n",
                ".align 2\n",
                ".globl ",
                ::core::stringify!($handler_name),
                "\n",
                ::core::stringify!($handler_name),
                ":\n",
                "    addi sp, sp, -4\n",
                "    sw   ra, 0(sp)\n",
                "    jal  ",
                ::core::stringify!($rust_fn),
                "\n",
                "    lw   ra, 0(sp)\n",
                "    addi sp, sp, 4\n",
                "    mret\n"
            )
        );
    };
}
