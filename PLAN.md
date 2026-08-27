# PLAN.md

A Rust rewrite of the Telegram export analyser. The exporter's job ends when
the files are on disk; this begins there — point it at a finished export and
it writes one `report.html` beside it.

The Python original stays. It is not being retired at parity: it keeps running,
which is what makes it usable as an oracle — and since the report was ported
faithfully rather than redesigned, that now covers the HTML as well as the
numbers (see **Verification**).

**All five phases are done.** The whole Python analyser is ported, and phase 5
went on top of it: `tga <folder>` writes the redesigned report — search, event
filters, a coverage band, compact presence rows — and `tga <folder> --classic`
writes the document `report.py` writes, byte for byte. That second flag is why
the oracle did not have to expire; see **Verification**.

## Commands

```powershell
save.bat                    # menu; also takes an action as an argument
save.bat save               # test + commit + push
save.bat test               # fmt + clippy + every suite
save.bat build              # release, then both exes into dist\
save.bat run                # the window
save.bat report <folder>    # write report.html beside an export
save.bat oracle             # re-record the Python side for both corpora
save.bat parity             # diff both classic reports against the Python's
save.bat bless              # re-record the two committed goldens
save.bat clean              # report target\ and empty it
```

`save.bat` sets **`TGA_REQUIRE_CORPUS=1`**, so on this machine a missing corpus
is a failure rather than a silent skip; a plain `cargo test --all` still skips,
which is what keeps a fresh clone working. Underneath it is plain cargo:

```powershell
cargo test --all
cargo test -p tga-report --test parity          # the HTML oracle
cargo test -p tga-report --test golden          # both goldens, no corpus needed
cargo test -p tga-metrics --test oracle         # the numbers oracle
cargo run  -p tga-app --bin TelegramAnalyser    # the window

cargo run -p tga-cli --bin tga -- <folder> [--out P] [--digest] [--no-fonts]
                                           [--stats P] [--notes P] [--classic]
cargo run -p tga-cli --bin tga -- --from-stats <stats.json> --out <report.html>
```

**`save.bat build` ships to `dist\`**, both binaries from one
`cargo build --release`. They live there rather than in `target\` because
`cargo clean` empties `target\` — and `save.bat clean` is a menu entry two rows
down — and because the window and the CLI are the same reader, the same metrics
and the same writer: a `dist\` holding one of them from Tuesday and the other
from Friday is a folder that can disagree with itself about what a report looks
like. Unlike `telegram_rust`'s `dist\`, nothing is written *beside* these
executables; see `dist\.gitkeep`.

**`--from-stats` opens no export**, and it exists because the layering already
made it free: `tga-report` depends on neither the reader nor the metrics, so a
recorded dump is the whole input. It is how the 42 existing notes were checked
against the real KRGM archive without copying 173 MB of other people's
conversation to put an `events.json` beside it.

## The two references, and what each one is for

**`C:\Users\Kosta\Projekti\telegram\analyser`** — 4,053 lines across 14
modules, plus the 590-line `app/ui/theme.py` it borrows. This is the *look* and
the *statistics*.

Read the implementation, not the docstring. The same rule the exporter's
CLAUDE.md sets applies here for the same reason: the comments in `read.py`,
`identity.py` and `palette.py` are the record of what went wrong the first
time, and a paraphrase loses exactly the defensive detail that was added after
something broke. Three that must survive the port, each of which changes a
number rather than a wording:

- **A forum topic is a thread**, so Telegram marks every top-level message in
  one as a reply to the message that opened it. Read literally, 2,702 of the
  UA KOLAB export's 3,518 "replies" become answers to a single message and the
  median response time stretches to days.
- **Time is two clocks.** Anything with a calendar or a clock face on it reads
  `date`, the export's naive local wall clock, because "who posts at 3am" is a
  question about the wall. Anything measuring a *duration* reads
  `date_unixtime`, which stays monotonic across a DST change where `date` does
  not.
- **A sender is a typed peer key, never a name.** Grouping by display name
  splits one person across every name they ever had, and merges two people who
  picked the same one.

**`J:\temp pureraw\KRGM TREĆI 2026.06.09\_timeline`** — 1,080 lines of JS, 312
of CSS, 153 of Python. This is the *data surface to reach*: a hub, a shared-axis
timeline carrying hand-written notes, a people table, a stats page, and search
across all three. Take what it shows. Leave its chrome.

## Decisions taken

| | |
|---|---|
| input | one export folder, as the Python analyser takes. Not `_timeline`'s multi-folder archive. |
| output | one self-contained `report.html`, as now. |
| look | the analyser's Swiss/International language, carrying `_timeline`'s data. |
| notes file | `_timeline`'s layout — it is the only one with anything written in it. |
| language | the **exporter** asks; the analyser follows what the export says. |
| repo | standalone cargo workspace. No path dependencies on `telegram_rust`. |
| the Python app | kept indefinitely, and kept running. |
| the report | **a faithful port of `report.py` first.** Decided 2026-08-27, and it is what made the HTML checkable: the Python's own `report.html` becomes an oracle exactly as its `stats.json` was. The `_timeline` data surface below is the pass after it. |
| the window | **in scope, and built.** Decided 2026-08-27 — see phase 4. |
| appearance | **dark only.** Decided 2026-08-27. One palette in `tga-ui`, one `html.dark` block in the report, no switch in either. See below. |

## Dark only

There is one appearance. `tga-ui` has one `Palette`, the report emits one
`html.dark` block, and neither offers a switch — a light theme is a second
design to keep in step, and this one has two colours and a red to keep in step
already.

**`tga_report::palette` still carries a light set, and that is not a
leftover.** The *classic* render is a byte-for-byte reproduction of
`report.py`, which had a theme switch and wrote both blocks, and the parity legs
compare that stylesheet character for character. A frozen reproduction is not a
second product; it is the oracle, and it gets to keep whatever the original had.
`golden-classic.html` pins the difference in both directions.

One thing this had to handle rather than merely delete: the frozen script
restores a theme from `localStorage` under `tg-report-theme`, and an older
report may well have written `light` there. With no `html.light` block and no
switch to get back, restoring it would render a page with no colours at all —
so the surface script removes the class and forgets the key.

## What "less cluttered" means

**Done, and it turned out to cost almost nothing** — because the five below are
things `_timeline` does that the *analyser's own design* never did. Phase 2
shipped a faithful port of `report.py`, which already had no cards, one hue and
no repeated chrome; phase 5 then took `_timeline`'s remaining data surface
(search, event filters, coverage) into that design rather than the other way
round. The list stayed a specification of what **not** to carry across, and
`tga-report` carries none of it.

Five specific things in `_timeline` that do not survive the port, each visible
in `_timeline\_work\shot-*.png`:

1. **Boxes.** Every stat tile, channel card and chart panel sits in a bordered,
   rounded box, and several nest two deep. The design this is going into is
   hairline rules and square corners; `report.py` states it outright — *there
   are no cards*.
2. **Hues.** `_timeline` runs a blue accent, four note-kind colours
   (green/blue/pink/orange) and a blue sequential ramp. This design is two
   colours and one red. The kinds are already carried by shape — ● ■ ▲ ◆ — and
   `_timeline`'s own README records why: normal-vision ΔE 19.3 but CVD ΔE 6.9,
   which forced shape-coding regardless. The colour is redundant; the shapes
   stay.
3. **Repeated chrome.** The date range appears in the nav bar and again under
   the page title. "Sve teme" sits beside the kind filters that already do
   that job.
4. **Scale mismatches.** Day-of-week is seven numbers given seven bars 150px
   tall.
5. **Cell borders and gaps** in the hour-of-day heatmap, which turn a field
   into a grid of buttons.

What replaces them is already specified by `palette.py`: one hue does all the
work, because almost every figure here is *magnitude*, whose correct encoding
is a single-hue sequential ramp. Where something genuinely needs telling apart
by identity, small multiples on a shared axis — not invented hues.

The ramp is generated in OKLab rather than eyeballed, holding the accent's hue,
chroma clamped to the sRGB gamut at each step. It passes four ordinal checks in
both modes and the port has to keep passing them:

```
                 light                dark      floor
monotone L       yes                  yes
adjacent dL      0.073                0.082     0.06
end vs surface   2.37:1               2.33:1    2:1
hue spread       1 degree             1 degree
```

`palette::tests` re-derives all four from the hex values rather than restating
them, so an edit that breaks one fails the suite instead of quietly shipping a
ramp nobody can read. **Both columns are still checked**, dark-only report or
not: the light ramp is what the classic render emits, and a ramp the oracle
carries is a ramp worth keeping correct.

## Repo shape

Standalone workspace. Layering enforced by the build, as in `telegram_rust`:

```
tga-read      export folder -> model. Both layouts: ours writes result.json per
              topic at the root, Desktop writes chats/chat_<id>/result.json,
              and both load without a flag. No UI, no network.
tga-notes     the hand-written annotation layer, and the digest a model reads.
              MUST NOT depend on tga-read — see below.
tga-metrics   every figure, one pass over the export. No I/O.
tga-report    the ramp, the SVG marks, and the one HTML file.
              MUST NOT depend on tga-read or tga-metrics.
tga-cli       the `tga` binary.
tga-ui        tokens, fonts and components in GPUI, ported from tgx-ui.
tga-app       the window. `TelegramAnalyser.exe`.
```

That `tga-report` rule is the one that pays for itself. The writer renders from
the metrics structure alone — a `serde_json::Value` of exactly the shape
`--stats` dumps — so a recorded fixture replays through it with no export on
disk. That is the same property that makes `tgx-html`'s parity leg possible, and
here it is what both the golden file and the HTML oracle below rest on.

**`tga-notes` inherits the rule at one remove.** It carries the `Event` type
that `tga-report` renders, so a dependency on the reader there would reach
`tga-report` transitively and cost exactly what the rule was drawn to keep.
`write_digest` therefore takes a plain `digest::Row` and the caller does the
mapping from `Export` — four lines, in `tga-cli` and in `tga-app`, and the only
price the rule charges anywhere.

## The one-file budget

Self-contained means one file: fonts base64'd into the stylesheet, every chart
inline SVG, no folder beside it and no request to anything. An archive is
something you keep, and a report that needs a CDN is a report that stops
working the year the CDN does.

"More data" does not threaten that, because the data is aggregated before it is
embedded. `_timeline` compresses 333,582 messages into 168 KB. The budget:

```
type          ~260 KB   three faces: Geist Regular, Medium, Geist Mono Regular
data          ~200 KB   aggregated, never raw messages
markup + SVG  the rest
                        target: under 1 MB for a 350k-message archive
```

For scale, the Python report on the KRGM export is already 1.5 MB, and the
`--digest` dump of the same export is 46.1 MB. Nothing resembling the latter
goes in the file.

**Measured, 2026-08-27. The budget is met.**

```
              classic      surface     read+render     Python's own timing
UA KOLAB       543 KB       496 KB        0.17 s       0.3 s
KRGM         1,487 KB       842 KB        5.73 s       24.8 s  (21.4 in the read)
```

The classic column is the port and is the Python's size by construction. The
surface column is what `tga <folder>` writes, and 842 KB on a 333,582-message
archive is inside the 1 MB target with the type still embedded.

**All of the saving came from one place, and it was worth measuring before
guessing.** On KRGM the file broke down as 275 KB of type, 275 KB of everything
on the page, and **969 KB of presence rows** — 260 of them, one per person and
per topic, at 64% of the document. Nothing else was worth touching. Three
changes to that one chart took it to 301 KB:

- the per-cell `fill` became five CSS rules;
- the per-cell `<rect>` became one `<path>` per shade, so a row is at most five
  elements however long the archive is;
- the per-cell `data-tip` became one `data-n` on the row, with the bucket labels
  written into the document once.

None of them drops a number, the chart is still drawn entirely server-side, and
the last one improves what it shrinks: hover now answers anywhere along a row,
including over the silent days, where before there was no element to hover.

**Interactivity is the real tension.** Pan, zoom, kind filters and search are
what make `_timeline` worth opening, and the Python report ships about twenty
lines of JS. The JS may grow to a few hundred lines, on one condition: every
view is rendered server-side into the SVG, and JS only filters and pans what is
already drawn. A report that renders blank with scripting off has stopped being
an archive.

**Speed.** The Python version reads 6,643 messages in 0.3s and 333,582 in
24.8s — 21.4 of that in the JSON read, 3.3 counting, 0.1 rendering. The read is
the whole cost and it is the part Rust should flatten. Anything above a couple
of seconds on KRGM means the model is being built wrong, not that the archive
is big.

## The language handshake

The exporter asks the user; the analyser follows what the export says. Since
these are now separate applications, the answer has to be recorded *inside the
export folder* for the analyser to find it.

It cannot go in `result.json`. That file is byte-exact with Telegram Desktop's
own output, key order included, and the entire parity harness rests on it — one
added key and the json leg fails on every message. It goes in a sidecar at the
export root instead.

A Telegram Desktop export has no such sidecar and never will, so a missing file
is the normal case, not an error: fall back to English and say nothing.

Both halves are work on the exporter's side, not this repo's, and neither
blocks anything here — the report is built with the strings behind one lookup
from the start, so wiring the sidecar in is a later, mechanical change.

## The notes file

Counting tells you how much was said. It cannot tell you what any of it meant,
so that part is not computed: it is written by hand, in a file beside the
export, and the report pins each entry onto the timeline as a marker.

The layout is `_timeline`'s, because it is the only one with real content in
it — `events/general.json`, 34.7 KB, 42 written notes covering General from
2025-09-05 to 2025-10-05. The Python analyser's own layout has never been
filled in for any export: its `--digest` pass has run on KRGM, leaving
`analysis/digest.jsonl` and `analysis/EVENTS.md`, but nothing was ever written
back from them.

```json
{ "coverage": { "from": "2025-09-05", "to": "2025-10-05",
                "level": "prekretnice", "note": "..." },
  "events": [ { "d": "2025-09-27", "t": "01:00", "title": "...", "text": "...",
                "kind": "pokret|organizacija|drama|produkcija",
                "w": "major|medium|minor",
                "who": ["name exactly as it appears in the export"],
                "tags": ["..."] } ] }
```

Two properties of the Python version carry over unchanged, because both were
learned the hard way:

- **Nothing in it is trusted.** It is written outside the program, so every
  string is escaped on the way into the HTML like any other hostile value, and
  a malformed entry is dropped rather than raising. An unreadable annotation
  must not cost you the report.
- **A `who` that names nobody says so.** If a name in the file matches no one
  in the statistics, the panel reports that rather than breaking — it means the
  export spells the name differently, which is a thing the reader can fix.

`coverage` is the field the Python layout has no way to express, and it is the
one worth having: a timeline with markers over one month and nothing over the
next nine looks identical whether the rest was quiet or simply unread.

## Verification

An earlier draft of this section said the fidelity target removed the byte
oracle: nothing could diff this report against the Python one, because it was
deliberately not the same document. **That was true of the redesign and it is
not true of what was built.** Deciding to port `report.py` faithfully first
bought the oracle back, and it is the single most valuable thing about that
decision — the HTML is now checked the same way the numbers are, on the same two
archives, rather than by looking at it.

**The numbers get a real oracle, and it costs the Python repo nothing.**
`tools/dump_python_stats.py` imports `analyser.metrics.analyse` and writes what
it returns. The other checkout is not modified — an earlier version of this
added a `--stats` flag over there and that was the wrong place for it: needing
something to check against is *this* repo's problem, so the code for it is this
repo's code.

**It is scaffolding with an expiry date.** It works because the two
implementations are currently supposed to agree. The moment the Rust analyser
computes something the Python one never did — which is the entire point of the
rewrite — a difference stops meaning "Rust is wrong" and this stops being an
oracle.
**It was going to expire at phase 5, and it did not have to.** One `bool` on
`tga_report::Options` avoided it: `classic: true` renders the document
`report.py` renders and the parity legs pass it, while `tga <folder>` renders
the redesign. The classic path is frozen, every phase-5 addition sits behind an
`if !classic`, and `golden.rs` fails if one leaks — so two million characters go
on being checked against a working implementation instead of against my own
reasoning. Two real corpora exist:

| corpus | messages | topics |
|---|---:|---|
| `N:\telegram export\UA KOLAB TELEGRAM` | 6,643 | 4 |
| `J:\temp pureraw\KRGM TREĆI 2026.06.09` | 333,582 | 10 |

That checks 4,000 lines of ported statistics against a working implementation
instead of against my own reasoning, on real data, including all three of the
traps named at the top.

**The HTML gets the same treatment, and it is the same two files over again.**
`tools/dump_python_report.py` renders the Python's `report.html`;
`tools/diff_report.py` compares it from outside and
`crates/tga-report/tests/parity.rs` does it inside `cargo test`. One divergence
is declared and printed on every run: the interaction graph's `<svg>` is masked,
because its coordinates are the `graph/nodes[]/{x,y}` drift phase 1 already ruled
out. The mask is by *element* rather than by attribute, and the harness fails if
it matched nothing — masking only the numbers would leave a chart that had
stopped being drawn at all reading as a match.

```
              compared     result
UA KOLAB       525,833     identical
KRGM         1,487,717     identical
synthetic       51,538     identical   (committed; never skips)
```

**The third leg is there because neither real archive has an `events.json`.**
Both corpora prove the statistics and the nine sections, and both leave the
entire annotation layer's markup — the rail, the markers, the cards, the
citations, the "skipped as malformed" note — uncompared. `tests/synthetic/` is a
four-message export written by hand with an events file beside it, and it is
committed rather than gitignored because it names nobody: the leg therefore runs
on a fresh clone with no drives, no corpus and no Python. It loads the events
through `tga_notes::load` rather than hand-building them, so the loader is under
the oracle as well as the writer, and the file carries one deliberately
malformed entry that both sides must drop *and* count.

**The HTML gets two golden files**, and they are **synthetic** rather than
recorded — which is forced, not chosen: `reference/` and `*.stats.json` are
gitignored because they are other people's conversation, so a golden cut from a
real archive could not be committed at all.
`crates/tga-report/tests/fixture.stats.json` is hand-written to the shape
`analyse` returns and carries the awkward cases on purpose — a name with `&` and
`<` in it, a person who never spoke, an alias list, a non-ASCII topic, a
superlative whose value is a string, an export with no roster dates. They catch
drift on a fresh clone with no corpus, no drives and no Python. They do not
claim correctness, and should not be described as if they do; the parity leg is
what does that.

One of them, `golden-classic.html`, does something the parity legs cannot: it
fails if a phase-5 addition **leaks into the classic render**. That is the whole
load-bearing claim of the `classic` flag, and without a check it would be a
claim maintained by care. The check matches on *markup* rather than on class
names, because the stylesheet names `.kind-filter` and `.cov-read` whether or
not anything uses them — matching those would let a feature that renders nothing
pass on the strength of its own CSS.

**Neither corpus goes into git.** Both are verbatim chat history from real
people. Same treatment as `telegram_rust`'s `reference/`: gitignored, sha256
manifest, and a *loud* skip when absent — a passing test that silently ran
nothing is how three green legs missed four missing features in the exporter.

## Phases

| | | done when | |
|---|---|---|---|
| 1 | `tga-read` + `tga-metrics` + `--stats`. No HTML. | Rust's `stats.json` matches Python's on both corpora. | **done** |
| 2 | the report: tokens, ramp, SVG marks, the sections. | Rust's `report.html` matches Python's on both corpora. | **done** |
| 3 | notes + digest. | an events file lands on the timeline; `--digest` writes what a model reads. | **done** |
| 4 | the GPUI window. | folder picker, drag-and-drop, progress, open the report. | **done** |
| 5 | the redesign — `_timeline`'s data surface, "less cluttered", the size budget. | search, event filters and coverage land; KRGM renders under 1 MB; the 42 existing notes land on the timeline. | **done** |

Phase 2's "done when" is **not** what this table originally said. It read
"render inside the size budget", which was a target for a document that does not
exist yet; a faithful port renders at the Python's size by construction, and the
condition that actually meant something was the diff. The budget moved to phase
5, where it belongs.

### Phase 1, as it actually went

**12 of 12 branches match on both corpora** — 6,643 messages over 4 topics, and
333,582 over 10. `--stats` was added to the Python analyser to produce the
oracle; `tools/diff_stats.py` compares the two from outside and
`crates/tga-metrics/tests/oracle.rs` does the same inside `cargo test`, so a
regression fails the ordinary suite.

Three things worth keeping:

- **The big corpus earned its place.** The small one passed every branch
  first time. KRGM found the one real difference — two service messages carry
  `"members": [null]`, and Python's `str(None)` names an arrival after the
  four-character string "None". A corpus of 6,643 messages would never have
  shown it.
- **Two divergences are declared, not discovered.** That null member, and the
  force-layout coordinates. Both are printed on every diff run with the reason
  each was ruled out, because "the diff is noisy" and "the port is wrong" look
  identical until somebody checks. The layout one was checked: `graph::tests`
  runs the same 220-step schedule on a four-node graph and matches Python to
  1e-12, which is what distinguishes chaotic amplification of `hypot`'s last
  bit from a mistranslation.
- **Speed: 32s to 6.5s** on KRGM, against Python's own timing of 21.4s of that
  32 inside the JSON read. The read was the whole cost and it is the part that
  went away.

### Phases 2 to 4, as they actually went

**The whole Python analyser is ported.** `palette.py`, `charts.py`, `report.py`,
`events.py`, `main.py` and `window.py` — the remaining 2,519 lines of the 4,053
— became `tga-report`, `tga-notes`, the `tga` flags, `tga-ui` and `tga-app`.
Four things worth keeping:

- **The port bought the oracle back**, and that was the whole argument for doing
  it before the redesign. 1,487,717 characters of the KRGM report compare
  identical to the Python's, with one declared mask. Nothing about "same look,
  more data" could have been checked that way.
- **One upstream defect is carried on purpose.** `report.py:246` ships
  `content:'<U+0091>2 '` in the `details[open]` marker — a C1 control character
  followed by an ASCII `2`, which is what a `−` becomes after a bad encoding
  round trip, and which a browser renders as a stray `2`. `tga-report` emits it
  byte-for-byte so the diff stays a clean zero, with a test that fails if
  somebody quietly fixes it. It is a one-line change and it belongs in the
  declared-divergence list, not slipped in under a port.
- **The window borrows exactly one control.** `gpui-component` is in the tree
  for the path field and nothing else: a single-line field with a caret, a
  selection and a clipboard is the one piece where hand-rolling produces
  something visibly worse, and pasting a path out of Explorer is how this window
  is actually used. It costs 13.6 MB — `TelegramAnalyser.exe` is 15.2 MB against
  `tga.exe`'s 1.6 — and that is the honest price of the decision.
- **The bar is determinate, always.** It first shipped passing `None` when idle,
  which `progress_bar` paints as a 12% indeterminate marker — so a window that
  had been asked to do nothing read as a run already under way. Every stage here
  is known and counted; there was never anything for `None` to mean.

### Phase 5, as it actually went

**The notes layout was the real gap, and it was in phase 3.** This file has said
from the start that the notes file uses `_timeline`'s layout — `d`, `t`, `text`,
`w`, `who`, `tags` and a `coverage` block — because it is the only one with
anything written in it. Phase 3 shipped `events.py`'s layout instead, which no
export has ever had a file for. `tga-notes` now reads both, field by field, with
no mode flag: `date`/`d`/`start` are all a date, `summary`/`text` are both the
body, and a file mixing them is somebody editing by hand rather than an error.

The 42 notes in `_timeline/events/general.json` load **verbatim, with no
conversion**, and every one of them lands:

```
42 markers, 42 cards
filters   ● organizacija 12   ■ drama 10   ▲ pokret 13   ◆ produkcija 7
coverage  Read 5 Sep 2025 - 5 Oct 2025 · 31 of 275 days · prekretnice
who       42 lines, 6 names matching nobody in the export
```

Four things worth keeping:

- **The `who` check earned its place on the first real file.** PLAN.md asked for
  "a `who` that names nobody says so"; run against the archive those notes were
  written about, six of the credited names match no one — `јелена`, `Димитрије
  ФФ`, `Mil'ca FFUBG` and three more. That is not a bug in either file, it is
  the export spelling them differently, and `_timeline`'s own `events/_ljudi.md`
  exists because somebody hit exactly this by hand. The report now points at it
  instead of leaving it to be noticed.
- **Coverage is the field that changes what the page means.** "31 of 275 days"
  turns ten months of empty timeline from *quiet* into *unread*, which is the
  opposite claim, and no amount of counting could have told them apart.
- **Kinds are carried by shape, not by hue** — ● ■ ▲ ◆, built from the
  vocabulary the file actually used rather than a fixed four, so a file naming
  three kinds gets three controls and one naming a word nobody anticipated still
  gets a filter. `_timeline`'s README records why colour was not an option:
  normal-vision ΔE 19.3 against CVD ΔE 6.9.
- **The oracle survived the phase that was supposed to end it.** See
  **Verification**; it cost one `bool` and a golden.

## Still open

- The exporter side of the language handshake: which sidecar file, and whether
  the exporter asks per-export or once as a setting. The report is already built
  with its strings behind one lookup, so wiring it in stays mechanical. This is
  the last thing on the list that is not this repo's to do.
- **The notes still only cover one month of one topic of one archive.** The
  machinery is finished and proven; what is missing is writing more of them.
  `tga <folder> --digest` produces what a model reads, `analysis/EVENTS.md`
  tells it the shape, and `--from-stats ... --notes ...` re-renders in a second
  without touching the export.
- The `<U+0091>2` in the `details[open]` marker is still carried in the
  **classic** render, on purpose, so the parity diff stays a clean zero. The
  surface render overrides it with a real minus, because there it is simply a
  bug with no diff to answer to. Fixing the classic one too is a one-line change
  and a declared entry in `parity.rs`'s `MASKED` list — worth doing only
  alongside a re-record of the Python side, or the two will disagree.
- No `origin` remote. `save.bat push` says so and stops rather than guessing.
