# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Rust rewrite of a Python Telegram-export analyser: point `tga` at a finished
export folder and it writes one self-contained `report.html` beside it.
`TelegramAnalyser.exe` is the same program with a GPUI window on it.

**Read `PLAN.md` first.** It is the record of every decision, why it was taken,
and how each phase actually went. All five phases are done; the open items are
listed at its end.

## Commands

`save.bat` is the entry point — bare for a menu, or with an action:

```powershell
save.bat test               # fmt + clippy + every suite (what `save` runs)
save.bat save               # test + commit + push
save.bat build              # release, both exes into dist\
save.bat run                # the window
save.bat report <folder>    # write report.html beside an export
save.bat oracle             # re-record the Python side for both corpora
save.bat parity             # diff both classic reports against the Python's
save.bat bless              # re-record the two committed goldens
save.bat clean              # report target\ size, then empty it
```

Underneath it is plain cargo. Single legs:

```powershell
cargo test -p tga-report  --test golden    # goldens; no corpus, no Python needed
cargo test -p tga-report  --test parity    # the HTML oracle
cargo test -p tga-metrics --test oracle    # the numbers oracle
cargo test -p tga-read    --test corpus    # the reader against a real export
cargo test --all -- --nocapture            # --nocapture matters; see below

cargo run -p tga-cli --bin tga -- <folder> [--out P] [--digest] [--no-fonts]
                                           [--stats P] [--notes P] [--classic]
cargo run -p tga-cli --bin tga -- --from-stats <stats.json> --out <report.html>
```

- **`TGA_REQUIRE_CORPUS=1`** (which `save.bat` sets) turns a missing corpus or
  recorded oracle into a failure instead of a skip. A plain `cargo test --all`
  still skips, which is what keeps a fresh clone working. The skips are
  `eprintln`s, so always pass `-- --nocapture` — libtest discards them otherwise
  and a suite that compared nothing reports as coverage.
- **`TGA_BLESS=1 cargo test -p tga-report --test golden`** re-records the two
  goldens. Read the diff before committing it.
- `TGA_EXPORT` overrides the reader test's corpus path.

## Layering, and the two rules that pay for themselves

```
tga-read      export folder -> model. Both layouts (ours: result.json per topic
              at the root; Desktop: chats/chat_<id>/result.json). No UI, no network.
tga-notes     the hand-written annotation layer + the model-facing digest.
              MUST NOT depend on tga-read.
tga-metrics   every figure, one pass. No I/O.
tga-report    the ramp, the SVG marks, the one HTML file.
              MUST NOT depend on tga-read or tga-metrics.
tga-cli       the `tga` binary.
tga-ui        tokens, fonts, components in GPUI.
tga-app       the window, TelegramAnalyser.exe.
```

`tga-report` renders from a `serde_json::Value` of exactly the shape
`tga_metrics::analyse` returns and `--stats` dumps. That is what lets a recorded
fixture replay through the writer with no export on disk — the golden test, the
parity test and `--from-stats` all rest on it. `tga-notes` inherits the rule at
one remove because it carries the `Event` type `tga-report` renders; the
`Export -> digest::Row` mapping therefore lives in the *callers*
(`tga-cli/src/main.rs`, `tga-app`).

## Invariants that change numbers or break the harness

- **Time is two clocks.** Anything with a calendar or clock face reads
  `Msg::when` (naive local wall clock); anything measuring a *duration* reads
  `Msg::unix`, which stays monotonic across DST. Decided once in `tga-read`.
- **A sender is a typed peer key, never a display name.** Grouping by name
  splits one person across their renames and merges two people who share one.
- **A forum topic is a thread**, so every top-level message in one is marked as
  a reply to the message that opened it. Read literally, response-time medians
  stretch to days.
- **`serde_json` without `preserve_order`.** Its default BTreeMap serialises
  sorted, matching Python's `sort_keys=True`; turning it on fills the stats diff
  with reordering noise.
- **`Options::classic` is what keeps the oracle alive.** `classic: true` renders
  the document `report.py` renders, byte for byte, and the parity legs compare
  it. Every post-port addition sits behind `if !classic`; `golden-classic.html`
  fails if one leaks. Do not "tidy" the classic path — including the deliberate
  `\u{91}2` defect in the `details[open]` marker, which is reproduced so the
  parity diff stays a clean zero and has a test guarding it.
- **A figure the Python analyser never computed goes in a declared branch.**
  `tga_metrics::ADDED` (mirrored in `tools/diff_stats.py`) names the branches
  that are Rust-only; `dynamics` is the one so far. A branch on that list may be
  absent from the Python dump and **must** be — if a name appears on both sides
  the carve-out is hiding a real difference and `oracle.rs` fails. Adding a new
  *key inside an existing branch* is not covered by this and will fail the diff
  as `only in rust`; that needs a re-record of the Python side instead.
- **Nothing in a notes file is trusted.** Every string is escaped into the HTML
  and a malformed entry is dropped, never raised — which is why `tga_notes::load`
  returns no `Result`.
- **One file, no network.** Fonts base64'd into the stylesheet, charts inline
  SVG, every view rendered server-side; JS only filters and pans what is already
  drawn. Target: under 1 MB for a 350k-message archive.

## Verification

Two real corpora on removable drives (`N:\telegram export\UA KOLAB TELEGRAM`,
`J:\temp pureraw\KRGM*`) plus the Python original at
`C:\Users\Kosta\Projekti\telegram` (its `.venv`, not system python) are the
oracle for both the numbers and the HTML. `tools/dump_python_*.py` record it
into `reference/`; `tools/diff_*.py` compare from outside and the `oracle`/
`parity` tests do the same inside `cargo test`.

Divergences are **declared, never discovered**: `EXCLUDED` in
`tga-metrics/tests/oracle.rs` (per field), `tga_metrics::ADDED` (per branch, for
figures with no Python counterpart) and `MASKED` in
`tga-report/tests/parity.rs`. Each entry carries the reason it was ruled out,
and all three lists are printed on every run. `EXCLUDED` and `ADDED` are
mirrored in `tools/diff_stats.py`; adding to one and not the other makes the two
harnesses disagree about what is being checked.

`reference/`, `report*.html` and `*.stats.json` are gitignored — they are
verbatim chat history from real people. The committed fixtures
(`tga-report/tests/fixture.stats.json`, `tests/synthetic/`) are hand-written and
name nobody, which is why they can be committed and never skip. They catch
drift; they do not claim correctness — the parity leg is what does that.

`.gitattributes` forces `-text` on every golden and recorded file (a CRLF
checkout would fail every byte-for-byte compare), `eol=crlf` on `*.bat` (cmd's
`goto` cannot find a label in an LF-only batch file), and `binary` on `*.ttf`.

## Conventions

- Comments here record *what went wrong* and *what was ruled out*, not what the
  code does. Preserve them through edits; a paraphrase loses the defensive
  detail that was added after something broke. New non-obvious decisions get the
  same treatment.
- The Python original is the reference for behaviour — read its implementation,
  not its docstrings.
- Dark only. One `Palette` in `tga-ui`, one `html.dark` block in the surface
  report, no switch. The light set in `tga_report::palette` is not a leftover:
  the classic render emits it, and `palette::tests` re-derives all four ordinal
  ramp checks from the hex values in both modes.
- Pre-1.0 dependencies (`gpui`, `gpui-component`) are pinned with `=`, and moved
  deliberately on their own commit with the parity legs green either side.
- Toolchain is pinned: Rust 1.97.0, MSVC target.
- No `origin` remote; `save.bat push` says so and stops.
