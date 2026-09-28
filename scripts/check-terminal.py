#!/usr/bin/env python3
"""Fast Linux PTY regression: exit keys, termios recovery, no mouse/clipboard capture.

Run after cargo build --locked --release. No physical hardware is accessed.
"""
import fcntl
import os
import pty
import select
import struct
import subprocess
import termios
import time

for key in [b"q", b"\x03", b"\x1b"]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 25, 80, 0, 0))
    before = termios.tcgetattr(slave)
    process = subprocess.Popen(
        ["target/release/shr-lux"], stdin=slave, stdout=slave, stderr=slave
    )
    try:
        output = b""
        deadline = time.monotonic() + 5
        while b"DISABLED" not in output and time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                output += os.read(master, 65536)
        assert b"DISABLED" in output, "Startup did not render"
        os.write(master, key)
        process.wait(timeout=5)
        while select.select([master], [], [], 0)[0]:
            output += os.read(master, 65536)
        assert process.returncode == 0
        assert termios.tcgetattr(slave) == before, "Terminal settings not restored"
        assert b"\x1b[?1049l" in output, "Alternate screen not restored"
        for sequence in [
            b"\x1b[?1000h", b"\x1b[?1002h", b"\x1b[?1003h", b"\x1b]52;"
        ]:
            assert sequence not in output, "Unexpected mouse/clipboard capture"
        print(f"PTY startup/exit/restoration PASS: {key!r}")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)
