#!/usr/bin/env python3
"""End to end input delay of the real wend binary, with no terminal emulator.

Runs wend on a pseudo terminal, types keys at it, and times how long until the frame
that answers each one has been written back. A frame ends with the synchronized update
terminator (ESC [ ? 2026 l), so that is what the timer waits for. What it leaves out is
the terminal emulator parsing and painting those bytes.

    scripts/e2e_latency.py [--binary PATH] [--file PATH | --size BYTES] [--cols N --rows N]
"""

import argparse
import fcntl
import os
import select
import statistics
import struct
import subprocess
import sys
import tempfile
import termios
import time

FRAME_END = b"\x1b[?2026l"

LINES = [
    "fn layout(buffer: &Buffer, area: Area) -> Layout {",
    "    let width = area.width().min(100); // keep lines readable",
    "    for (row, slot) in slots.iter().enumerate() {",
    "        // 漢字とかなが混ざったコメント",
    '        let name = "café naïve résumé 🦀";',
    "\tif slot.is_blank() {",
    "    }",
    "",
    '    let message = "a line long enough to wrap on any terminal, so scrolling and '
    "moving through it go through the wrapped row paths of the layout, which a file of "
    'short lines never touches, and it keeps going a little longer to wrap twice";',
]


def source_text(size):
    text = ""
    while len(text.encode()) < size:
        text += "\n".join(LINES) + "\n"
    return text


class Wend:
    def __init__(self, binary, path, cols, rows):
        self.master, slave = os.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self.process = subprocess.Popen(
            [binary, path],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"},
            start_new_session=True,
        )
        os.close(slave)

    def read(self, timeout):
        """Returns what wend wrote within `timeout` seconds, empty when nothing came."""
        if not select.select([self.master], [], [], timeout)[0]:
            return b""
        try:
            return os.read(self.master, 1 << 20)
        except OSError:
            return b""

    def settle(self, quiet=0.3):
        """Reads until wend has been quiet for `quiet` seconds."""
        while self.read(quiet):
            pass

    def send(self, keys):
        os.write(self.master, keys.encode())

    def delay(self, key):
        """Seconds from sending `key` to the end of the first frame that follows."""
        start = time.perf_counter()
        self.send(key)
        tail = b""
        while True:
            chunk = self.read(5)
            if not chunk:
                raise SystemExit("no frame came back for a key press, is wend running?")
            tail = (tail + chunk)[-(len(FRAME_END) + len(chunk)) :]
            if FRAME_END in tail:
                elapsed = time.perf_counter() - start
                self.settle(0.02)
                return elapsed

    def flood(self, key, count):
        """Sends `count` keys in one write. Returns seconds until wend goes quiet and the
        number of frames it drew meanwhile."""
        start = time.perf_counter()
        self.send(key * count)
        frames, last = 0, start
        tail = b""
        while True:
            chunk = self.read(0.2)
            if not chunk:
                return last - start, frames
            data = tail + chunk
            seen = data.count(FRAME_END)
            if seen:
                frames += seen
                last = time.perf_counter()
            tail = data[-(len(FRAME_END) - 1) :]

    def close(self):
        self.send("\x1bq")
        try:
            self.process.wait(2)
        except subprocess.TimeoutExpired:
            self.process.kill()
        os.close(self.master)


def summary(samples):
    ms = sorted(sample * 1000 for sample in samples)
    p95 = ms[min(len(ms) - 1, int(len(ms) * 0.95))]
    return f"median {statistics.median(ms):7.2f}  p95 {p95:7.2f}  min {ms[0]:7.2f}  max {ms[-1]:7.2f}"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", default="target/release/wend")
    parser.add_argument("--file", help="open this file, default a generated source file")
    parser.add_argument("--size", type=int, default=1_000_000, help="bytes to generate")
    parser.add_argument("--cols", type=int, default=120)
    parser.add_argument("--rows", type=int, default=40)
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as directory:
        path = args.file
        if path is None:
            path = os.path.join(directory, "bench.rs")
            with open(path, "w") as file:
                file.write(source_text(args.size))

        print(f"{args.binary} on {path}, {args.cols}x{args.rows}, times in ms")
        phases = [("type", "i", "x", 25), ("down", "", "j", 100), ("page down", "", "J", 20)]
        for name, setup, key, presses in phases:
            wend = Wend(args.binary, path, args.cols, args.rows)
            wend.settle(1.0)
            wend.send(setup)
            wend.settle()
            samples = [wend.delay(key) for _ in range(presses)]
            print(f"input delay, {name:9}  {summary(samples)}")
            wend.close()

        wend = Wend(args.binary, path, args.cols, args.rows)
        wend.settle(1.0)
        elapsed, frames = wend.flood("j", 200)
        print(
            f"200 keys at once      {elapsed * 1000:7.1f} ms until quiet, "
            f"{frames} frames drawn, {frames / elapsed:.0f} frames/s"
        )
        wend.close()


if __name__ == "__main__":
    sys.exit(main())
