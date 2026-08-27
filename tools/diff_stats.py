r"""Diff the Rust analyser's --stats output against the Python original's.

This is the oracle for phase 1. The report is deliberately not the Python
report, so nothing can diff the HTML -- but the *numbers* under it are a port,
and a port either reproduces them or does not.

    python tools/diff_stats.py reference/ua-kolab.stats.json reference/rust.stats.json

Branches the Rust side has not implemented yet are reported as outstanding
rather than as failures, and branches present on one side only are always a
failure: a missing branch that reads as "no differences" is exactly how a port
gets declared finished early.

Floats are compared with a tolerance, because both sides round in their own
language before serialising and an exact match on a ratio is luck, not
correctness. Everything else is compared exactly.
"""
from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

TOLERANCE = 1e-9
MAX_SHOWN = 25

#: Fields that are allowed to differ, and why. Every entry needs a reason that
#: says what was ruled out, because "the diff is noisy" and "the port is wrong"
#: look identical until somebody checks. Printed on every run: an exclusion
#: nobody sees is an exclusion that grows.
EXCLUDED = {
    "graph/nodes[]/x": "force-layout coordinate — see below",
    "graph/nodes[]/y": "force-layout coordinate — see below",
    "churn/events[]/names[]": "a null member — see below",
}

EXCLUSION_NOTE = """\
     graph node coordinates are 220 iterations of Fruchterman-Reingold, and
     the two implementations disagree in the third decimal on a 28-node graph.
     This is drift, not a mistranslation: `graph::tests` runs the same layout
     on a four-node graph, where nothing can amplify, and matches Python to
     1e-12 at 1, 5 and all 220 steps. What differs is the last bit of
     `math.hypot` against `f64::hypot`, which a chaotic n-body schedule turns
     into a visible number. The coordinates encode nothing that `edges` and
     `degree` do not already carry, and both of those are compared.

     churn event names: two service messages in the KRGM export carry
     `"members": [null]`. Python builds that list with `str(m)`, so the null
     becomes the four-character string "None" and the report names an arrival
     after it. The Rust side reads it as an empty name instead. This is the one
     place the port deliberately does not reproduce the original, and it costs
     no number: `count` is `len(members) or 1` either way, so every arrival
     total still matches -- which the diff confirms, since only the name
     differs.\
"""


def load(path: Path) -> dict:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


def normalised(path: str) -> str:
    """`graph/nodes[3]/x` -> `graph/nodes[]/x`, so one entry covers every node."""
    return re.sub(r"\[\d+\]", "[]", path)


def compare(a, b, path: str, out: list[str]) -> None:
    if len(out) > MAX_SHOWN:
        return
    if normalised(path) in EXCLUDED:
        return
    if isinstance(a, dict) and isinstance(b, dict):
        for key in sorted(set(a) | set(b)):
            if key not in a:
                out.append(f"{path}/{key}: only in python")
            elif key not in b:
                out.append(f"{path}/{key}: only in rust")
            else:
                compare(a[key], b[key], f"{path}/{key}", out)
        return
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"{path}: length {len(a)} vs {len(b)}")
            return
        for i, (x, y) in enumerate(zip(a, b)):
            compare(x, y, f"{path}[{i}]", out)
        return
    if isinstance(a, bool) != isinstance(b, bool):
        out.append(f"{path}: {a!r} vs {b!r}")
        return
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if not math.isclose(a, b, rel_tol=TOLERANCE, abs_tol=TOLERANCE):
            out.append(f"{path}: {a!r} vs {b!r}")
        return
    if a != b:
        out.append(f"{path}: {a!r} vs {b!r}")


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__)
        return 2
    python_side, rust_side = (load(Path(p)) for p in argv)

    print("     excluded from this diff:")
    for field, why in sorted(EXCLUDED.items()):
        print(f"       {field}  --  {why}")
    print(EXCLUSION_NOTE)
    print()

    branches = sorted(set(python_side) | set(rust_side))
    failures = 0
    outstanding: list[str] = []

    for branch in branches:
        if branch not in rust_side:
            outstanding.append(branch)
            continue
        if branch not in python_side:
            print(f"FAIL {branch}: the Rust side emits a branch Python does not")
            failures += 1
            continue
        out: list[str] = []
        compare(python_side[branch], rust_side[branch], branch, out)
        if out:
            failures += 1
            print(f"FAIL {branch}  ({len(out)}{'+' if len(out) > MAX_SHOWN else ''} differences)")
            for line in out[:MAX_SHOWN]:
                print(f"       {line}")
        else:
            print(f"ok   {branch}")

    if outstanding:
        print(f"\n     not ported yet: {', '.join(outstanding)}")
    print()
    if failures:
        print(f"{failures} branch(es) differ")
        return 1
    done = len(branches) - len(outstanding)
    print(f"{done}/{len(branches)} branches match")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
