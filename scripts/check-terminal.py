#!/usr/bin/env python3
"""Fast Linux PTY regressions after the release build. No hardware or recordings."""
import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import wave


def check(args, action):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 25, 80, 0, 0))
    before = termios.tcgetattr(slave)
    process = subprocess.Popen(
        ["target/release/shr-lux", *args], stdin=slave, stdout=slave, stderr=slave
    )
    try:
        output = b""
        deadline = time.monotonic() + 5
        while b"DISABLED" not in output and time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                output += os.read(master, 65536)
        assert b"DISABLED" in output, "Startup did not render"
        if isinstance(action, bytes):
            os.write(master, action)
        elif action == "SIGTERM":
            process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
        while select.select([master], [], [], 0)[0]:
            output += os.read(master, 65536)
        assert process.returncode == 0, output
        assert termios.tcgetattr(slave) == before, "Terminal settings not restored"
        assert b"\x1b[?1049l" in output, "Alternate screen not restored"
        for sequence in [
            b"\x1b[?1000h", b"\x1b[?1002h", b"\x1b[?1003h", b"\x1b]52;"
        ]:
            assert sequence not in output, "Unexpected mouse/clipboard capture"
        print(f"PTY PASS: {args[0] if args else 'setup'} / {action!r}")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


for key in [b"q", b"\x03", b"\x1b"]:
    check([], key)
with tempfile.TemporaryDirectory() as directory:
    path = os.path.join(directory, "aux.wav")
    with wave.open(path, "wb") as audio:
        audio.setparams((4, 2, 48000, 0, "NONE", "not compressed"))
        audio.writeframes(bytes(48000 * 8))
    for action in [b"q", b"\x03", b"\x1b", "SIGTERM", "EOF"]:
        check(["simulate", "metal", "--file", path], action)
