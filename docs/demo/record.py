#!/usr/bin/env python3
"""Record a qe-monitor demo as an asciicast v2 file.

A real vc-relax takes minutes, so this replays a finished output file into a
temp file while qe-monitor follows it, then cycles the right-hand plot panel.

Writes an asciicast; render it to a GIF with docs/demo/render.sh, which holds
the exact agg flags.
"""

import fcntl
import json
import os
import pty
import signal
import select
import struct
import sys
import termios
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SOURCE = os.path.join(REPO, "tests/fixtures/pw/vc_relax.out")
BINARY = os.path.join(REPO, "target/release/qe-monitor")

COLS, ROWS = 120, 36
SEED_LINES = 126  # header only: enough for qe-monitor to accept the file
REPLAY_START = 0.8  # let the first frame settle
REPLAY_SECONDS = 4.0  # how long the "run" appears to take
# Left panel stays on |dE|; cycle the right one to Pressure, then Force.
KEYS = [(5.1, "3"), (6.0, "2")]
# Stop recording while Force is still on screen - quitting would clear the
# alternate screen and leave a blank final frame. Render with
# `agg --last-frame-duration 3` to hold that view before the GIF loops.
STOP_AT = 6.5

# ionic-step boundaries in the source file; each one adds a point to the charts
STEPS = [261, 411, 564, 717, 861, 1005, 1149, 1299, 1449, 1600, 1910]


def feed(target, lines):
    """Append the run to `target`, pacing it so each ionic step lands evenly."""
    chunks, prev = [], SEED_LINES
    for end in STEPS:  # 4 sub-chunks per step so the SCF
        step = lines[prev:end]  # iterations also appear gradually
        size = max(1, len(step) // 4)
        for i in range(0, len(step), size):
            chunks.append(step[i : i + size])
        prev = end
    delay = REPLAY_SECONDS / len(chunks)
    time.sleep(REPLAY_START)
    with open(target, "a") as fh:
        for chunk in chunks:
            fh.write("".join(chunk))
            fh.flush()
            os.fsync(fh.fileno())
            time.sleep(delay)


def main():
    out_cast = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "demo.cast")
    target = os.path.join(HERE, "vc-relax.out")

    lines = open(SOURCE).readlines()
    with open(target, "w") as fh:
        fh.writelines(lines[:SEED_LINES])

    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.chdir(HERE)  # keeps the title bar short
        os.execv(BINARY, ["qe-monitor", os.path.basename(target)])

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    threading.Thread(target=feed, args=(target, lines), daemon=True).start()

    events, start, pending = [], time.time(), list(KEYS)
    while True:
        now = time.time() - start
        if now >= STOP_AT:
            break
        while pending and now >= pending[0][0]:
            os.write(fd, pending.pop(0)[1].encode())
        r, _, _ = select.select([fd], [], [], 0.02)
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            events.append([round(time.time() - start, 4), "o", data.decode("utf-8", "replace")])

    # drain the last repaint, then kill the app rather than letting it exit
    # cleanly, so no screen-clearing escape sequence is recorded
    deadline = time.time() + 0.3
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.1)
        if not r:
            break
        try:
            data = os.read(fd, 65536)
        except OSError:
            break
        if not data:
            break
        events.append([round(time.time() - start, 4), "o", data.decode("utf-8", "replace")])

    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)

    header = {"version": 2, "width": COLS, "height": ROWS, "timestamp": int(start), "env": {"TERM": "xterm-256color"}}
    with open(out_cast, "w") as fh:
        fh.write(json.dumps(header) + "\n")
        for e in events:
            fh.write(json.dumps(e) + "\n")
    os.unlink(target)
    print(f"{out_cast}: {len(events)} events, {events[-1][0] if events else 0:.1f}s")


if __name__ == "__main__":
    main()
