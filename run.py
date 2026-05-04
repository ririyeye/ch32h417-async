"""Flash and attach RTT to CH32H417 via probe-rs."""
import subprocess, sys, os

PROBE_RS = os.path.join("..", "probe-rs", "target", "release", "probe-rs.exe")
CHIP = "CH32H417"
RTT_ARGS = ["--no-catch-reset", "--no-catch-hardfault"]

def main():
    if len(sys.argv) < 2:
        print("Usage: run.py <elf_file>")
        sys.exit(1)
    elf = sys.argv[1]

    # Flash
    print("-- Flashing --")
    subprocess.run([PROBE_RS, "download", "--chip", CHIP, "--chip-erase", elf], check=True)

    # Attach RTT
    print("-- Attaching RTT --")
    subprocess.run([PROBE_RS, "attach", "--chip", CHIP, *RTT_ARGS, elf], check=True)

if __name__ == "__main__":
    main()
