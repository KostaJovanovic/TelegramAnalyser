# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Point `TelegramAnalyser.exe` at a finished Telegram export folder and it writes
one self-contained `report.html` beside it. Run it with no arguments and the
same program opens a window instead.

**One executable.** It shipped as two — the window, and `tga.exe` for the
command line — until the only difference between them turned out to be how they
were launched, which argv already records.

**Read `PLAN.md` first.** It is the record of why each figure is computed the
way it is, and of the decisions that would otherwise be re-litigated every time
somebody reads the code. `REFACTOR.md` is the log of the rewrite that produced
the current shape — worth reading once, for what it found.

## Commands

`save.bat` is the entry point — bare for a menu, or with an action:

```powershell
save.bat test               # fmt + clippy + every suite (what `save` runs)
save.bat save               # test + commit + push
save.bat build              # release, both exes into dist\
save.bat run                # the window
save.bat report <folder>    # write report.html beside an export
save.bat baseline           # both archives against the last recording
save.bat baseline record    # re-record it
save.bat bless              # re-record the committed golden
save.bat clean              # report target\ size, then empty it
```

Underneath it is plain cargo. Single legs:

```powershell
cargo test -p tga-report --test golden     # the golden; no corpus needed
cargo test -p tga-read   --test corpus     # the reader against a real export
cargo test --all -- --nocapture            # --nocapture matters; see below

cargo run -p tga-app --bin TelegramAnalyser -- <folder>
        [--out P] [--digest] [--no-fonts] [--stats P] [--notes P] [--stamp T]
cargo run -p tga-app --bin TelegramAnalyser -- --from-stats <s.json> --out <p>
```

- **`TGA_REQUIRE_CORPUS=1`** (which `save.bat` sets) turns a missing corpus into
  a failure instead of a skip. A plain `cargo test --all` still skips, which is
  what keeps a fresh clone working. The skips are `eprintln`s, so always pass
  `-- --nocapture` — libtest discards them otherwise and a suite that compared
  nothing reports as coverage.
- **`TGA_BLESS=1 cargo test -p tga-report --test golden`** re-records the
  golden. Read the diff before committing it.
- `TGA_EXPORT` overrides the reader test's corpus path.

## Layering, and the two rules that pay for themselves

```
tga-read      export folder -> model. Both layouts (ours: result.json per topic
              at the root; Desktop: chats/chat_<id>/result.json). No UI, no network.
tga-notes     the hand-written annotation layer + the model-facing digest.
              MUST NOT depend on tga-read.
tga-stats     the shape of every figure, and the dump's format. Data only.
tga-metrics   every figure, one pass. No I/O. Fills in a tga_stats::Stats.
tga-report    the ramp, the SVG marks, the one HTML file. Reads a Stats.
              MUST NOT depend on tga-read or tga-metrics.
tga-app       the only binary, TelegramAnalyser.exe. cli.rs is the argument
              half; app.rs is the window, egui on glow; state.rs holds every
              rule the window applies, with no toolkit in it, so they stay
              testable.
```

`tga-report` renders from a `tga_stats::Stats`, which is exactly what `--stats`
dumps. That is what lets a recorded dump replay through the writer with no
export on disk — the golden test and `--from-stats` both rest on it. **The
shape lives in `tga-stats` rather than in either side**, so the writer can read
it without depending on the reader or the metrics, and the compiler checks every
field name. `tga-notes` inherits the rule at one remove because it carries the
`Event` type `tga-report` renders; the `Export -> digest::Row` mapping therefore
lives in the *caller* (`tga-app/src/cli.rs`).

Every field of `Stats` has a `Default` and the struct is `#[serde(default)]`, so
a dump with a branch missing renders an empty section rather than failing to
load. `dynamics` is an `Option` on purpose: absent is "recorded before this
existed, draw no section", which is a different fact from an archive with
nothing in it.

## Invariants that change numbers or break the harness

- **Time is two clocks.** Anything with a calendar or clock face reads
  `Msg::when` (naive local wall clock); anything measuring a *duration* reads
  `Msg::unix`, which stays monotonic across DST. Decided once in `tga-read`.
- **A sender is a typed peer key, never a display name.** Grouping by name
  splits one person across their renames and merges two people who share one.
- **A forum topic is a thread**, so every top-level message in one is marked as
  a reply to the message that opened it. Read literally, response-time medians
  stretch to days.
- **The stats dump is sorted by `tga_stats::write`, not by the map type.**
  `serde_json`'s object is a `BTreeMap` and sorts itself — until something turns
  on its `preserve_order` feature, and cargo unifies features across everything
  built in one invocation. The window's old toolkit turned it on, so building
  the command-line binary alone and building the workspace produced differently
  ordered dumps from the same numbers. The sort is explicit now; do not remove it on the
  grounds that BTreeMap already does it, and do not assume the current
  dependency set is the last one that will reach for that feature.
- **`assets/report.css` and `assets/report.js` are copied into the report
  verbatim**, comments included. Editing either changes the file's bytes and
  fails `save.bat baseline`, and `.gitattributes` pins their line endings for
  the same reason it pins the golden's.
- **`Count` serialises as `["label", 41]`, not as an object.** Nine branches use
  it. The `from`/`into` pair on the struct is what keeps the file's shape while
  the code reads `.label` and `.n`; changing it invalidates every recorded dump.
- **Nothing in a notes file is trusted.** Every string is escaped into the HTML
  and a malformed entry is dropped, never raised — which is why `tga_notes::load`
  returns no `Result`.
- **One file, no network.** Fonts base64'd into the stylesheet, charts inline
  SVG, every view rendered server-side; JS only filters and pans what is already
  drawn. Target: under 1 MB for a 350k-message archive.

## Verification

**`save.bat baseline` is the load-bearing check.** It runs the program over both
real archives and compares the whole report and the whole stats dump, byte for
byte, against a recording. A difference is a mistake rather than a judgement
call unless the change was meant to alter the output — and then it is
`save.bat baseline record`, after reading the diff. Three legs: `ua-kolab`,
`krgm`, and `krgm-notes`, which passes the 42 hand-written notes so the whole
annotation layer's markup is covered too.

Two real corpora, both on removable drives:

| corpus | messages | topics |
|---|---:|---|
| `N:\telegram export\UA KOLAB TELEGRAM` | 6,643 | 4 |
| `J:\temp pureraw\KRGM*` | 333,582 | 10 |

Re-record with `save.bat baseline record` **only** for a change that is meant to
alter the output, and read the diff first.

`baseline/`, `reference/`, `report*.html` and `*.stats.json` are gitignored —
they are verbatim chat history from real people. (`reference/` holds output from
the deleted Python harness; nothing reads it any more and it can be removed.)
The committed fixtures (`tga-report/tests/fixture.stats.json`,
`tests/synthetic/`) are hand-written and name nobody, which is why they can be
committed and never skip. They catch drift on a fresh clone with no drives; they
do not claim correctness.

`.gitattributes` forces `-text` on every golden and recorded file (a CRLF
checkout would fail every byte-for-byte compare), `eol=crlf` on `*.bat` (cmd's
`goto` cannot find a label in an LF-only batch file), and `binary` on `*.ttf`.

## Conventions

- Comments here record *what went wrong* and *what was ruled out*, not what the
  code does. Preserve them through edits; a paraphrase loses the defensive
  detail that was added after something broke. New non-obvious decisions get the
  same treatment.
- Dark only. One palette in `tga-app/src/theme.rs`, one `html.dark` block in the
  report, no switch. `tga_report::palette::tests` re-derives all four ordinal
  ramp checks from the hex values, and `tga_app::theme::tests` asserts the
  window's copy has not drifted from the report's.
- The window is `eframe`/`egui` on the **glow** backend, deliberately: it is a
  text field, two tick boxes, a bar and three buttons, and wgpu drags in a
  shader compiler for that. Everything the design needs and egui does not
  default to — square corners, hairline strokes, no shadow, Geist — is set in
  `tga_app::theme::install` rather than inherited.
- Toolchain is pinned: Rust 1.97.0, MSVC target.
- No `origin` remote; `save.bat push` says so and stops.
