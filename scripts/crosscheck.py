#!/usr/bin/env python3
"""Independent Python SHA-256 Challenge Lock and bit-exact kernel cross-check."""

from __future__ import annotations

import hashlib
import pathlib
import subprocess
import sys


def extract_blocks(path: pathlib.Path) -> dict[str, str]:
    blocks: dict[str, str] = {}
    active_id: str | None = None
    buf: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        t = raw.strip()
        marker = None
        for prefix in ("//", "--"):
            if t.startswith(prefix):
                rest = t[len(prefix) :].strip()
                if rest.startswith("@comparator:begin "):
                    marker = ("begin", rest[len("@comparator:begin ") :].strip())
                elif rest.startswith("@comparator:end "):
                    marker = ("end", rest[len("@comparator:end ") :].strip())
        if marker and marker[0] == "begin":
            assert active_id is None
            active_id = marker[1]
            buf = []
        elif marker and marker[0] == "end":
            assert active_id == marker[1]
            payload = "".join(line + "\n" for line in buf).encode("utf-8")
            blocks[active_id] = hashlib.sha256(payload).hexdigest()
            active_id = None
        elif active_id is not None:
            buf.append(raw.rstrip("\r"))
    assert active_id is None
    return blocks


def main() -> int:
    repo = pathlib.Path(__file__).resolve().parent.parent
    rust_pins = extract_blocks(repo / "src/kernels.rs")
    lean_pins = extract_blocks(repo / "lean/ComparatorOpt/Challenge.lean")
    lean_pins.update(extract_blocks(repo / "lean/ComparatorOpt/Check.lean"))

    lock_entries: dict[tuple[str, str], str] = {}
    for raw in (repo / "comparator.lock").read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line.startswith("pin "):
            _, kind, pid, digest, _rel = line.split()
            lock_entries[(kind, pid)] = digest

    assert len(lock_entries) == 7
    assert len(rust_pins) + len(lean_pins) == 7
    for pid, digest in rust_pins.items():
        assert lock_entries[("rust-ref", pid)] == digest, f"mismatch on rust-ref {pid}"
    for pid, digest in lean_pins.items():
        assert lock_entries[("lean-challenge", pid)] == digest, f"mismatch on lean-challenge {pid}"

    # Verify CLI end-to-end exit status.
    subprocess.check_call(
        ["cargo", "run", "--release", "--quiet", "--bin", "comparator-opt", "--", "verify", "."],
        cwd=repo,
    )
    print("crosscheck: ok (independent hashlib.sha256 lock verification and CLI verify passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
