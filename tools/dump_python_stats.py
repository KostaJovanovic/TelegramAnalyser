r"""Dump the *Python* analyser's figures as JSON, without modifying it.

This lives here, not there. The Python analyser is a separate program that
happens to compute the same numbers, and the only reason this repo cares is
that a reimplementation needs something to be checked against. That is this
repo's problem, so the code for it is this repo's code -- the other checkout
stays exactly as it was.

    python tools/dump_python_stats.py "<export folder>" reference/<name>.stats.json

It imports `analyser.metrics.analyse` and calls it. Nothing is written into the
export folder and no report is rendered, so pointing it at somebody's archive
costs one read.

Needs the Python analyser's venv, because `analyser.read` and `analyser.metrics`
are plain modules but the package imports PySide6 on the window path:

    C:\Users\Kosta\Projekti\telegram\.venv\Scripts\python.exe tools/dump_python_stats.py ...

**This is scaffolding with an expiry date.** It exists to prove the port, and
it is useful only while the two implementations are meant to agree. Once the
Rust analyser starts computing things the Python one never did -- which is the
whole point of the rewrite -- the answer to a diff failure stops being
automatically "Rust is wrong", and this stops being an oracle. See PLAN.md,
"Verification".
"""
from __future__ import annotations

import json
import sys
from datetime import date, datetime
from pathlib import Path

#: Where the Python analyser lives. Overridable, because the only thing this
#: script really needs is a folder with an `analyser` package in it.
DEFAULT_SOURCE = Path(r"C:\Users\Kosta\Projekti\telegram")


def isodate(value: object) -> str:
    """JSON fallback for the only non-primitive the figures carry.

    Anything else raises rather than being stringified: a silent `str()` would
    turn a shape difference into a passing diff.
    """
    if isinstance(value, (date, datetime)):
        return value.isoformat()
    raise TypeError(f"{type(value).__name__} is not JSON: {value!r}")


def main(argv: list[str]) -> int:
    if not 2 <= len(argv) <= 3:
        print(__doc__)
        return 2

    folder = Path(argv[0]).expanduser()
    out = Path(argv[1]).expanduser()
    source = Path(argv[2]).expanduser() if len(argv) == 3 else DEFAULT_SOURCE

    if not folder.is_dir():
        print(f"Not a folder: {folder}")
        return 1
    if not (source / "analyser").is_dir():
        print(f"No analyser package under {source}")
        return 1

    sys.path.insert(0, str(source))
    from analyser.metrics import analyse
    from analyser.read import load

    export = load(folder, progress=lambda done, total, name: print(f"  read {done}/{total}  {name}"))
    if not export.msgs:
        print("That export has no messages in it.")
        return 1

    stats, _people = analyse(export)

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        # Sorted, because what is being verified is the numbers, not the key
        # order -- and sorting is what makes the two dumps comparable line for
        # line, since serde_json's default map sorts too.
        json.dumps(stats, indent=1, sort_keys=True, ensure_ascii=False, default=isodate),
        encoding="utf-8",
        newline="\n",
    )
    print(f"  {out}  ({len(export.msgs):,} messages, {len(export.topics)} topics)")
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    raise SystemExit(main(sys.argv[1:]))
