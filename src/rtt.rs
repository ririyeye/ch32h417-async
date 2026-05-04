//! RTT transport — raw string writes via write_volatile.
//! Channel name "Terminal" so probe-rs displays as plain text.

use core::ptr::write_volatile;

const BUF_SIZE: usize = 1024;

#[repr(C)]
struct RttChannel {
    name: *const u8,
    buffer: *mut u8,
    size: u32,
    write: u32,
    read: u32,
    flags: u32,
}

#[repr(C)]
struct RttControlBlock {
    id: [u8; 16],
    max_up_channels: u32,
    max_down_channels: u32,
    up_channel: RttChannel,
}

#[link_section = ".rtt_buf"]
static mut RTT_BUF: [u8; BUF_SIZE] = [0u8; BUF_SIZE];

#[link_section = ".rtt"]
#[no_mangle]
#[used]
static mut _SEGGER_RTT: RttControlBlock = RttControlBlock {
    id: [0; 16],
    max_up_channels: 0,
    max_down_channels: 0,
    up_channel: RttChannel {
        name: core::ptr::null(),
        buffer: core::ptr::null_mut(),
        size: 0,
        write: 0,
        read: 0,
        flags: 0,
    },
};

pub fn init() {
    unsafe {
        let cb = &raw mut _SEGGER_RTT;
        let addr = cb as *mut u32;
        write_volatile(addr, u32::from_le_bytes(*b"SEGG"));
        write_volatile(addr.add(1), u32::from_le_bytes(*b"ER R"));
        write_volatile(addr.add(2), u32::from_le_bytes(*b"TT\0\0"));
        write_volatile(addr.add(3), u32::from_le_bytes(*b"\0\0\0\0"));
        (*cb).max_up_channels = 1;
        (*cb).max_down_channels = 0;
        (*cb).up_channel.name = b"Terminal\0" as *const u8;
        (*cb).up_channel.buffer = &raw mut RTT_BUF as *mut u8;
        (*cb).up_channel.size = BUF_SIZE as u32;
        (*cb).up_channel.write = 0;
        (*cb).up_channel.read = 0;
        (*cb).up_channel.flags = 0;
    }
}

/// Write a single byte. Returns false if buffer full.
unsafe fn write_byte(b: u8) {
    let cb = &raw mut _SEGGER_RTT;
    let ch = &raw mut (*cb).up_channel;
    let next = ((*ch).write + 1) % (*ch).size;
    if next == (*ch).read {
        return;
    }
    write_volatile((*ch).buffer.add((*ch).write as usize), b);
    (*ch).write = next;
}

pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        unsafe {
            write_byte(b);
        }
    }
}

/// Write u32 as decimal, most-significant-digit first. No stack buffer.
pub fn write_u32(mut n: u32) {
    if n == 0 {
        unsafe {
            write_byte(b'0');
        }
        return;
    }
    // find the highest divisor
    let mut div: u32 = 1;
    while n / div >= 10 {
        div *= 10;
    }
    while div > 0 {
        let d = (n / div) as u8;
        unsafe {
            write_byte(b'0' + d);
        }
        n -= (d as u32) * div;
        div /= 10;
    }
}
