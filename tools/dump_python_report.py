r"""Render the *Python* analyser's `report.html`, without modifying it.

The sibling of `dump_python_stats.py`, and here for the same reason: the Rust
report is a faithful port of `analyser/report.py`, so the Python's own output is
an oracle for the HTML exactly as `stats.json` was for the numbers. Needing
something to check against is this repo's problem, so the code for it is this
repo's code -- the other checkout stays exactly as it was.

    python tools/dump_python_report.py "<export folder>" reference/<name>.html

Two arguments must match what `tga` is invoked with for the diff to mean
anything, and both are stamped into the Notes section:

  --stamp  the date the report says it was written on. Defaults to today, which
           is what both implementations do; pass it explicitly if the two runs
           could land on either side of midnight.
  --theme  dark or light. Defaults to dark, as both do.

Needs the Python analyser's venv, because `analyser.report` is a plain module
but the package imports PySide6 on the window path:

    C:\Users\Kosta\Projekti\telegram\.venv\Scripts\python.exe tools/dump_python_report.py ...

**Scaffolding with an expiry date**, like the stats dumper. It is an oracle only
while the two implementations are meant to agree; the moment the Rust report
shows something the Python one never did, a difference stops meaning "Rust is
wrong". See PLAN.md, "Verification".
"""
from __future__ import annotations

import sys
from pathlib import Path

#: Where the Python analyser lives. Overridable, because the only thing this
#: script really needs is a folder with an `analyser` package in it.
DEFAULT_SOURCE = Path(r"C:\Users\Kosta\Projekti\telegram")

#: What `report.render` stamps into the Notes section by default. The Rust side
#: uses the same string, so a diff does not turn on which program wrote it.
SOURCE_NAME = "Telegram Export Analyser"


def _pinned(text: str):
    """A stand-in for ``report.datetime`` whose clock reads ``text``.

    ``render`` calls ``datetime.now().strftime("%d %B %Y").lstrip("0")`` and
    does nothing else with it, so answering the format string with the wanted
    string is the whole of what has to be faked. It survives the ``lstrip``
    unchanged as long as the caller does not pass a date starting with a zero,
    which no formatted date does.
    """

    class _Fixed:
        @staticmethod
        def strftime(_fmt: str) -> str:
            return text

    class _Clock:
        @staticmethod
        def now(tz=None):                     # noqa: ARG004
            return _Fixed

    return _Clock


def main(argv: list[str]) -> int:
    positional: list[str] = []
    theme = "dark"
    stamp = ""
    embed = True
    source = DEFAULT_SOURCE

    rest = list(argv)
    while rest:
        flag = rest.pop(0)
        if flag == "--theme" and rest:
            theme = rest.pop(0).lower()
        elif flag == "--stamp" and rest:
            stamp = rest.pop(0)
        elif flag == "--no-fonts":
            embed = False
        elif flag == "--analyser" and rest:
            source = Path(rest.pop(0)).expanduser()
        elif flag.startswith("--"):
            print(f"Unknown option: {flag}\n{__doc__}")
            return 2
        else:
            positional.append(flag)

    if len(positional) != 2:
        print(__doc__)
        return 2

    folder = Path(positional[0]).expanduser()
    out = Path(positional[1]).expanduser()

    if not folder.is_dir():
        print(f"Not a folder: {folder}")
        return 1
    if not (source / "analyser").is_dir():
        print(f"No analyser package under {source}")
        return 1

    sys.path.insert(0, str(source))
    from analyser import events as events_mod
    from analyser import report as report_mod
    from analyser.metrics import analyse
    from analyser.read import load

    export = load(folder, progress=lambda done, total, name:
                  print(f"  read {done}/{total}  {name}"))
    if not export.msgs:
        print("That export has no messages in it.")
        return 1

    stats, people = analyse(export)
    found, note = events_mod.load(folder)

    # `report.render` reads the clock itself, so two runs either side of
    # midnight differ in the Notes line and nowhere else. The Rust side takes
    # the stamp as a parameter; here it has to be swapped in, because the whole
    # point of this script is that the other checkout is not edited.
    original = report_mod.datetime
    if stamp:
        report_mod.datetime = _pinned(stamp)
    try:
        html = report_mod.render(stats, people, found, note, theme=theme,
                                 embed_fonts=embed, source=SOURCE_NAME)
    finally:
        report_mod.datetime = original

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(html, encoding="utf-8", newline="\n")
    print(f"  {out}  ({len(html):,} chars, {len(export.msgs):,} messages, "
          f"{len(found)} events)")
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    raise SystemExit(main(sys.argv[1:]))
