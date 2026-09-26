#!/usr/bin/env python3
"""Runs palmrest fixtures through the Python daemon, read-only, to make the parity goldens.

    python3 python_reference.py [DAEMON_DIR]

For every tests/fixtures/parity_*.bin it writes tests/fixtures/parity/<name>.python: one line
per record, "<axis> <h> <v> <h_hi> <v_hi>", the Python ScrollAxisLock's axis after the record
and the ScrollDelta it returned. For every decode_*.bin it writes <name>.python with the
touches parse_report returned, "id x y state size" per touch, ";"-separated.

The daemon module is imported, never modified: bytecode writing is disabled so its directory
is left exactly as found.
"""

import os
import struct
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.join(HERE, "..", "fixtures")
DAEMON = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.local/lib/magic-mouse-kde")
sys.path.insert(0, DAEMON)

import magic_mouse_gestures as g  # noqa: E402

MAGIC = b"PALMREC\x01"


def records(path):
    data = open(path, "rb").read()
    assert data.startswith(MAGIC), path
    at = len(MAGIC)
    while at < len(data):
        ns, length = struct.unpack_from("<QH", data, at)
        yield ns, data[at + 10: at + 10 + length]
        at += 10 + length


def parity(path):
    lock = g.ScrollAxisLock(speed=22, lock_ratio=1.5, lock_timeout=0.25, acceleration_max=2.3, lock_threshold=40)
    assert g.SCROLL_VELOCITY_GAIN_MAX == 2.0, "run without SCROLL_* variables set"
    for ns, data in records(path):
        d = lock.update(g.parse_report(data), now=ns / 1e9)
        axis = lock.axis or "none"
        yield f"{axis} {d.horizontal} {d.vertical} {d.horizontal_hi_res} {d.vertical_hi_res}"


def touches(path):
    for _, data in records(path):
        # parse_report drops non-contacts; decode goldens want every block, so parse each.
        blocks = []
        for i in range(max(0, (len(data) - 14) // 8)):
            t = g.parse_touch(data, 14 + 8 * i)
            blocks.append(f"{t.id} {t.x} {t.y} {t.state} {t.size}")
        yield ";".join(blocks)


def main():
    out_dir = os.path.join(FIXTURES, "parity")
    os.makedirs(out_dir, exist_ok=True)
    for name in sorted(os.listdir(FIXTURES)):
        stem, ext = os.path.splitext(name)
        if ext != ".bin":
            continue
        if stem.startswith("parity_"):
            lines = parity(os.path.join(FIXTURES, name))
        elif stem.startswith("decode_") and not stem.startswith("decode_double"):
            lines = touches(os.path.join(FIXTURES, name))
        else:
            continue
        with open(os.path.join(out_dir, stem + ".python"), "w") as f:
            f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
