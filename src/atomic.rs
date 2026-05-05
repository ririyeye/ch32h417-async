//! Atomic helper libcalls for CH32H417 V3F core.
//!
//! V3F supports AMO but NOT LR/SC. With our custom target
//! (`max-atomic-width: 0`, `atomic-cas: false`, `+forced-atomics`),
//! LLVM generates `__atomic_*` libcalls for all atomic operations.
//! We implement them here using interrupt disable (csrci/csrsi mstatus.MIE)
//! for those that require atomicity; plain volatile for relaxed load/store.

use core::ptr::{read_volatile, write_volatile};

#[no_mangle]
unsafe extern "C" fn __atomic_load_4(ptr: *const u32, _order: i32) -> u32 {
    read_volatile(ptr)
}

#[no_mangle]
unsafe extern "C" fn __atomic_store_4(ptr: *mut u32, val: u32, _order: i32) {
    write_volatile(ptr, val)
}

#[no_mangle]
unsafe extern "C" fn __atomic_fetch_add_4(ptr: *mut u32, val: u32, _order: i32) -> u32 {
    core::arch::asm!("csrci mstatus, 0x8"); // disable MIE
    let old = read_volatile(ptr);
    write_volatile(ptr, old + val);
    core::arch::asm!("csrsi mstatus, 0x8"); // enable MIE
    old
}

#[no_mangle]
unsafe extern "C" fn __atomic_compare_exchange_4(
    ptr: *mut u32,
    expected: *mut u32,
    desired: u32,
    _weak: bool,
    _succ: i32,
    _fail: i32,
) -> bool {
    core::arch::asm!("csrci mstatus, 0x8"); // disable MIE
    let old = read_volatile(ptr);
    if old == read_volatile(expected) {
        write_volatile(ptr, desired);
        core::arch::asm!("csrsi mstatus, 0x8"); // enable MIE
        true
    } else {
        write_volatile(expected, old);
        core::arch::asm!("csrsi mstatus, 0x8"); // enable MIE
        false
    }
}
