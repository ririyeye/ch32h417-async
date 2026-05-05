"""Flash and attach RTT to CH32H417 via probe-rs."""
import subprocess, sys, os

PROBE_RS = os.path.join("..", "probe-rs", "target", "release", "probe-rs.exe")
CHIP = "CH32H417"
RTT_ARGS = ["--no-catch-reset", "--no-catch-hardfault"]
DEFAULT_TIMEOUT = 10  # seconds for RTT attach

def main():
    if len(sys.argv) < 2:
        print("Usage: run.py <elf_file> [timeout_sec]")
        sys.exit(1)
    elf = sys.argv[1]
    timeout = float(sys.argv[2]) if len(sys.argv) > 2 else DEFAULT_TIMEOUT

    # Recovery: wlink erase resets chip state via WCH firmware,
    # bypassing the Debug Module (which may be stuck after Ctrl+C).
    print("-- wlink erase (recovery) --")
    subprocess.run(["wlink", "erase"], check=True)

    # Flash
    print("-- Flashing --")
    subprocess.run([PROBE_RS, "download", "--chip", CHIP, "--chip-erase", elf], check=True)

    # Attach RTT
    print(f"-- Attaching RTT (timeout={timeout}s) --")
    try:
        subprocess.run([PROBE_RS, "attach", "--chip", CHIP, *RTT_ARGS, elf],
                       check=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        print("RTT attach timed out (OK).")

if __name__ == "__main__":
    main()
