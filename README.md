# ch32h417-async

Minimal async/await blink demo for **CH32H417** (QingKe-V5F RISC-V core) on the nanoCH32H417 dev board.

Uses a **hand-written async runtime** — no embassy, no RTOS, no alloc.  
Just Rust's built-in `Future` trait and a ~15-line polling executor.

## Hardware

- **Chip**: WCH CH32H417 (QingKe-V5F + V3F dual-core, V5F used)
- **Board**: nanoCH32H417
- **Debugger**: WCH-LinkE (RV mode)
- **LEDs**: PC2 and PC3 (on-board, active-high)

## Quick Start

```bash
# One-time setup
rustup target add riscv32imac-unknown-none-elf

# Clone with embassy submodule (for hacking on embassy itself)
git clone --recurse-submodules https://github.com/ririyeye/ch32h417-async
cd ch32h417-async

# Build & flash (one command)
cargo run --release
```

`cargo run` uses [probe-rs](https://github.com/probe-rs/probe-rs)  
(with [CH32H417 probe-assisted flash support](https://github.com/ririyeye/probe-rs))
at `../probe-rs/target/release/probe-rs` for flashing.

## Architecture

```
┌──────────────────────────────────────┐
│ cargo run --release                  │
│   └─ probe-rs download --chip CH32H417 │  ← flash + DMI ndmreset
│        --chip-erase --binary-format elf │
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│ _start (global_asm)                 │
│   └─ rust_main()                    │
│        └─ run(blink())  ← executor  │
│             └─ blink()  ← async fn  │
│                  └─ Delay::ms(500)  │
└──────────────────────────────────────┘
```

### Custom async runtime

| Component | Description |
|-----------|-------------|
| `run()` | Polling executor, ~15 lines. Busy-loops until future returns `Ready`. |
| `Delay` | Software-counter future. Each `poll()` decrements a counter. |
| Waker | No-op. Single-threaded cooperative multitasking. |

CH32H417's `rdcycle` hardware counter is inhibited by default (`mcountinhibit`).
`Delay` uses a software counter instead, calibrated for HSI ~8 MHz.

### Memory layout

| Region | Start | Size |
|--------|-------|------|
| FLASH | 0x08000000 | 480 KB |
| ITCM RAM | 0x200A0000 | 128 KB |

## Dependencies

| Crate | Why |
|-------|-----|
| `panic-halt` | Halt on panic |
| *(none else)* | Everything else is in `main.rs` |

## Embassy Submodule

`embassy/` is a submodule pointing to [ririyeye/embassy](https://github.com/ririyeye/embassy).
It's not currently used as a dependency (our custom runtime is lighter),
but is available for hacking on embassy support for CH32H417.

To use embassy crates from the submodule, add path dependencies:

```toml
[dependencies]
embassy-executor = { path = "embassy/embassy-executor", features = ["arch-riscv32", "executor-thread"] }
embassy-time = { path = "embassy/embassy-time" }
```

## Related

- [ch32h417-blink](https://github.com/ririyeye/ch32h417-blink) — bare-metal version (no async)
- [probe-rs CH32H417 adaptation](https://github.com/ririyeye/probe-rs) — flash tooling
- [nanoCH32H417](https://github.com/wuxx/nanoCH32H417) — official dev board SDK
- [wlink](https://github.com/ch32-rs/wlink) — WCH-Link protocol reference

## License

MIT
