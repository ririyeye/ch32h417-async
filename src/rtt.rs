//! RTT transport — control block in NOLOAD .rtt, buffer in .rtt_buf.
//! All fields initialised byte-by-byte at runtime in `init()`.

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
        core::ptr::write_volatile(addr, u32::from_le_bytes(*b"SEGG"));
        core::ptr::write_volatile(addr.add(1), u32::from_le_bytes(*b"ER R"));
        core::ptr::write_volatile(addr.add(2), u32::from_le_bytes(*b"TT\0\0"));
        core::ptr::write_volatile(addr.add(3), u32::from_le_bytes(*b"\0\0\0\0"));
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

pub fn write_str(s: &str) {
    unsafe {
        let cb = &raw mut _SEGGER_RTT;
        let ch = &raw mut (*cb).up_channel;
        for &b in s.as_bytes() {
            let write = core::ptr::read_volatile(&(*ch).write);
            let read = core::ptr::read_volatile(&(*ch).read);
            let next = (write + 1) % (*ch).size;
            if next == read {
                break;
            }
            core::ptr::write_volatile((*ch).buffer.add(write as usize), b);
            // Fence before updating write pointer on OoO cores (V5F)
            core::arch::asm!("fence w, w");
            core::ptr::write_volatile(&raw mut (*ch).write, next);
        }
        // Drain store buffer to make writes visible to debug probe
        core::arch::asm!("fence iorw, iorw");
    }
}

#[allow(dead_code)]
pub fn write_hex(v: u32) {
    let mut buf = [0u8; 10]; // "0x" + 8 hex digits
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..8 {
        let nibble = (v >> (28 - 4 * i)) & 0xF;
        buf[2 + i] = if nibble < 10 {
            b'0' + nibble as u8
        } else {
            b'a' + (nibble - 10) as u8
        };
    }
    let s = unsafe { core::str::from_utf8_unchecked(&buf) };
    write_str(s);
}
