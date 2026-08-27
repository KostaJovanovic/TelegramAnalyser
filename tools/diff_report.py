r"""Diff the Rust analyser's report.html against the Python original's.

The oracle for phase 2, and the sibling of `diff_stats.py`. The report is a
*faithful* port -- same nine sections, same stylesheet, same marks -- so the
Python's own output is something the Rust output either reproduces or does not.

    python tools/dump_python_report.py "<export>" reference/ua-kolab.html --stamp "27 August 2026"
    tga "<export>" --out reference/rust-ua.html --quiet
    python tools/diff_report.py reference/ua-kolab.html reference/rust-ua.html

`crates/tga-report/tests/parity.rs` runs the same comparison inside
`cargo test`, so a regression fails the ordinary suite without anyone
remembering to run this. What this adds is a readable account of *where* the
two differ, which an `assert_eq!` on a 1.5 MB string cannot give.

Both files must have been rendered with the same theme, the same font setting
and the same date stamp -- `--stamp` on the dumper is what makes the last of
those true across midnight.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

#: Elements that are allowed to differ, and why. Every entry needs a reason
#: that says what was ruled out, because "the diff is noisy" and "the port is
#: wrong" look identical until somebody checks. Printed on every run: an
#: exclusion nobody sees is an exclusion that grows.
MASKED = {
    "the interaction graph": r'<svg class="chart network".*?</svg>',
}

MASK_NOTE = """\
     the interaction graph's node coordinates are 220 iterations of
     Fruchterman-Reingold, and the two implementations disagree in the third
     decimal on a 28-node graph. This is the same divergence phase 1 declared
     for `graph/nodes[]/{x,y}`, and it is drift rather than a mistranslation:
     `graph::tests` runs the same layout on a four-node graph, where nothing
     can amplify, and matches Python to 1e-12 at 1, 5 and all 220 steps. What
     differs is the last bit of `math.hypot` against `f64::hypot`.

     The whole `<svg>` is masked rather than just its numbers, because every
     coordinate in it is derived from those positions. That is deliberately
     coarse, so the check below refuses to pass if the mask matched nothing:
     an element that stopped being drawn at all would otherwise read as a
     match.\
"""

#: Where the section boundaries are, so a difference can be named rather than
#: only located. Ordered as the document is.
SECTIONS = (
    "masthead", "whole", "rhythm", "people", "talk", "said",
    "churn", "topics", "records", "notes",
)


def mask(html: str) -> tuple[str, int]:
    hits = 0
    for pattern in MASKED.values():
        html, n = re.subn(pattern, "[MASKED]", html, flags=re.S)
        hits += n
    return html, hits


def section_at(html: str, index: int) -> str:
    """Which section of the document character ``index`` falls in."""
    found = "before the masthead"
    for name in SECTIONS:
        at = html.find(f'id="{name}"')
        if at == -1 or at > index:
            break
        found = name
    return found


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__)
        return 2

    left, right = (Path(p).expanduser() for p in argv)
    for path in (left, right):
        if not path.is_file():
            print(f"Not a file: {path}")
            return 1

    python = left.read_text(encoding="utf-8")
    rust = right.read_text(encoding="utf-8")

    print(f"python  {left}  {len(python):,} chars")
    print(f"rust    {right}  {len(rust):,} chars")
    print()
    print("Masked, and everything outside them is compared exactly:")
    for name in MASKED:
        print(f"  - {name}")
    print(MASK_NOTE)
    print()

    a, hits_a = mask(python)
    b, hits_b = mask(rust)
    if not hits_a or not hits_b:
        # A mask that matches nothing is a mask that has stopped masking, and
        # the run that discovers that must not be a passing one.
        print(f"FAIL: the mask matched {hits_a} element(s) on the python side and "
              f"{hits_b} on the rust side. It is no longer masking what it names.")
        return 1

    if a == b:
        print(f"IDENTICAL, apart from {len(MASKED)} masked element(s). "
              f"{len(a):,} characters compared.")
        return 0

    index = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
    print(f"DIFFERENT at character {index:,} of {len(a):,}, "
          f"in the {section_at(a, index)} section:")
    print()
    print(f"  python: ...{a[max(0, index - 160):index + 200]}...")
    print()
    print(f"  rust  : ...{b[max(0, index - 160):index + 200]}...")
    print()
    if len(a) != len(b):
        print(f"  (lengths differ by {abs(len(a) - len(b)):,} characters)")
    return 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    raise SystemExit(main(sys.argv[1:]))
