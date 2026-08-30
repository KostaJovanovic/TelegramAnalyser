# PLAN.md

Point `tga` at a finished Telegram export folder and it writes one
self-contained `report.html` beside it. `TelegramAnalyser.exe` is the same
program with a window on it.

This file is the record of **why each figure is computed the way it is**, and of
the decisions that would otherwise be re-litigated every time somebody reads the
code. `CLAUDE.md` is the working guide; `REFACTOR.md` is the log of the rewrite
that produced the current shape.

## What it does, and what it refuses to do

The exporter's job ends when the files are on disk; this begins there. It reads
`result.json`, counts, and writes one file.

| | |
|---|---|
| input | one export folder. Both layouts load without a flag: ours writes `result.json` per topic at the root, Telegram Desktop writes `chats/chat_<id>/result.json`. |
| output | one self-contained `report.html`, and optionally a `--stats` dump of every figure. |
| appearance | **dark only**, one hue, hairline rules, square corners. |
| network | none, ever. Not at read time, not in the report. |
| notes | a hand-written annotation file beside the export, optional. |

## Commands

```powershell
save.bat test               # fmt + clippy + every suite (what `save` runs)
save.bat save               # test + commit + push
save.bat build              # release, both exes into dist\
save.bat run                # the window
save.bat report <folder>    # write report.html beside an export
save.bat baseline           # both archives against the last recording
save.bat bless              # re-record the committed golden
save.bat clean              # report target\ size, then empty it
```

**`save.bat build` ships to `dist\`**, both binaries from one
`cargo build --release`. They live there rather than in `target\` because
`cargo clean` empties `target\` and `save.bat clean` is a menu entry two rows
down — and because the window and the CLI are the same reader, the same metrics
and the same writer: a `dist\` holding one of them from Tuesday and the other
from Friday is a folder that can disagree with itself about what a report looks
like.

**`--from-stats` opens no export.** `tga-report` depends on neither the reader
nor the metrics, so a recorded dump is the whole input. It is how the 42
existing notes were checked against the real KRGM archive without copying 173 MB
of other people's conversation to put an `events.json` beside it.

## Repo shape

```
tga-read      export folder -> model. No UI, no network.
tga-notes     the hand-written annotation layer, and the digest a model reads.
              MUST NOT depend on tga-read — see below.
tga-stats     the shape of every figure, and the dump's format. Data only.
tga-metrics   every figure, one pass over the export. No I/O.
tga-report    the ramp, the SVG marks, and the one HTML file.
              MUST NOT depend on tga-read or tga-metrics.
tga-app       TelegramAnalyser.exe, the only binary. Arguments run the
              analyser and print; no arguments open the window, in egui.
```

**The `tga-report` rule is the one that pays for itself.** The writer renders
from a `tga_stats::Stats` and nothing else, so a recorded dump replays through
it with no export on disk. The golden test and `--from-stats` both rest on that,
and so does the fact that the report can be changed and checked without a
333,582-message archive plugged in.

The shape lives in `tga-stats` rather than in either side, which is what lets
both depend on it without depending on each other. That crate is data and derives
and reaches nothing.

**`tga-notes` inherits the rule at one remove.** It carries the `Event` type
that `tga-report` renders, so a dependency on the reader there would reach
`tga-report` transitively. `write_digest` therefore takes a plain `digest::Row`
and the caller does the mapping from `Export` — four lines, in
`tga-app/src/cli.rs`, and the only price the rule charges anywhere.

## The three rules that change numbers

Each of these was learned by getting it wrong, and each moves a figure rather
than a wording.

**Time is two clocks.** Anything with a calendar or a clock face on it reads
`Msg::when`, the export's naive local wall clock, because "who posts at 3am" is
a question about the clock in the room. Anything measuring a *duration* reads
`Msg::unix`, which stays monotonic across a DST change where the wall clock does
not. Decided once, in `tga-read`.

**A sender is a typed peer key, never a display name.** Grouping by name splits
one person across every name they ever had, and merges two people who picked the
same one.

**A forum topic is a thread**, so Telegram marks every top-level message in one
as a reply to the message that opened it. Read literally, 2,702 of the UA KOLAB
export's 3,518 "replies" become answers to a single message and the median
response time stretches to days.

## Dark only, one hue

There is one appearance. `tga-app`'s `theme` has one palette, the report emits
one `html.dark` block, and neither offers a switch — a light theme is a second
design to keep in step, and this one has two colours and a red to keep in step
already.

**One hue does all the work**, and that costs nothing: almost every figure here
is *magnitude*, whose correct encoding is a single-hue sequential ramp anyway.
Where something genuinely needs telling apart by identity, the report uses small
multiples on a shared axis rather than inventing hues the design does not have.
Event kinds are carried by shape — ● ■ ▲ ◆ — for a measured reason: the four
colours the source timeline used scored ΔE 19.3 to normal vision and ΔE 6.9 to
colour-vision deficiency, which forced shape-coding regardless.

The ramp is generated in OKLab rather than eyeballed, holding the accent's hue
with chroma clamped to the sRGB gamut at each step. It passes four ordinal
checks, and `palette::tests` re-derives all four from the hex values so an edit
that breaks one fails the suite instead of quietly shipping a ramp nobody can
read:

```
monotone L       yes
adjacent dL      0.082   (floor 0.06)
end vs surface   2.33:1  (floor 2:1)
hue spread       1 degree
```

The end step is what sets the span: on near-black, anything darker than 2:1
stops being a mark and becomes surface. That is why the ramp runs past the
accent into deeper red rather than starting at it — the accent is a step of the
hue's ramp, not its end.

## What the design does not carry

Five things the source timeline did that this does not, each of them a decision
rather than an omission:

1. **Boxes.** Every stat tile, channel card and chart panel sat in a bordered,
   rounded box, several nested two deep. Here it is hairline rules and square
   corners. There are no cards.
2. **Hues.** A blue accent, four note-kind colours and a blue sequential ramp,
   replaced by two colours and one red. See above.
3. **Repeated chrome.** The date range in the nav bar and again under the page
   title; "all topics" beside the kind filters that already do that job.
4. **Scale mismatches.** Day-of-week is seven numbers; it does not get seven
   bars 150px tall.
5. **Cell borders and gaps** in the hour-of-day heatmap, which turn a field into
   a grid of buttons.

## The one-file budget

Self-contained means one file: fonts base64'd into the stylesheet, every chart
inline SVG, no folder beside it and no request to anything. An archive is
something you keep, and a report that needs a CDN is a report that stops working
the year the CDN does.

More data does not threaten that, because the data is aggregated before it is
embedded — never raw messages.

**Measured, and the budget is met:**

```
              report    stats dump   read+render
UA KOLAB      520 KB       229 KB       0.19 s
KRGM          868 KB       853 KB       5.5 s     (333,582 messages, 10 topics)
KRGM + notes  922 KB                              (with the 42 hand-written events)
```

**All of the saving came from one place, and it was worth measuring before
guessing.** Drawn the obvious way, KRGM broke down as 275 KB of type, 275 KB of
everything on the page, and **969 KB of presence rows** — 260 of them, one per
person and per topic, at 64% of the document. Nothing else was worth touching.
Three changes to that one chart took it to 301 KB:

- the per-cell `fill` became five CSS rules;
- the per-cell `<rect>` became one `<path>` per shade, so a row is at most five
  elements however long the archive is;
- the per-cell `data-tip` became one `data-n` on the row, with the bucket labels
  written into the document once.

None of them drops a number, the chart is still drawn entirely server-side, and
the last one improves what it shrinks: hover now answers anywhere along a row,
including over the silent days, where before there was no element to hover.

**Interactivity is the real tension.** Pan, filters and search are what make the
page worth opening. The rule that keeps it an archive: **every view is rendered
server-side into the SVG, and JS only filters and pans what is already drawn.** A
report that renders blank with scripting off has stopped being an archive.

**Speed.** The read is the whole cost — on KRGM it is most of the 5.5 s, and the
counting and rendering are under a second between them. Anything above a couple
of seconds there means the model is being built wrong, not that the archive is
big.

## The notes file

Counting tells you how much was said. It cannot tell you what any of it meant,
so that part is not computed: it is written by hand, in a file beside the
export, and the report pins each entry onto the timeline as a marker.

```json
{ "coverage": { "from": "2025-09-05", "to": "2025-10-05",
                "level": "prekretnice", "note": "..." },
  "events": [ { "d": "2025-09-27", "t": "01:00", "title": "...", "text": "...",
                "kind": "pokret|organizacija|drama|produkcija",
                "w": "major|medium|minor",
                "who": ["name exactly as it appears in the export"],
                "tags": ["..."] } ] }
```

Two layouts load, field by field, with no mode flag: `date`/`d`/`start` are all
a date, `summary`/`text` are both the body. A file mixing them is somebody
editing by hand rather than an error.

Three properties, each of which was earned:

- **Nothing in it is trusted.** It is written outside the program, so every
  string is escaped on the way into the HTML like any other hostile value, and a
  malformed entry is dropped rather than raising. An unreadable annotation must
  not cost you the report — which is why `tga_notes::load` returns no `Result`.
- **A `who` that names nobody says so.** Run against the archive those 42 notes
  were written about, six of the credited names match no one in the export. That
  is not a bug in either file; it is the export spelling them differently, which
  is a thing the reader can go and fix, and it is invisible unless the report
  points at it.
- **`coverage` is the field that changes what the page means.** "31 of 275 days"
  turns ten months of empty timeline from *quiet* into *unread*, which is the
  opposite claim, and no amount of counting could have told them apart.

## Beyond counting

`tga-metrics/src/dynamics/` holds five figures the counting elsewhere leaves
unanswerable:

| | | on KRGM |
|---|---|---|
| `pairs` | reply edges collapsed onto unordered pairs, ranked on `min(there, back)` rather than the total | 4,007 mutual, 2,355 one-way |
| `answer` | median reply latency by the hour the *parent* was posted | fastest 03:00 at 22 s, slowest 08:00 at 39 min |
| `tenure` | first, last, active days, days dormant, and a status per person | 137 active, 32 fading, 81 long gone |
| `retention` | active / new / returning / lost per month, silent months included | 10 months, 82.2% kept |
| `depth` | reply-chain length, 2 up to 8+ | deepest 22, mean 2.5 |

- **`pairs` is the figure the graph section cannot give.** A directed edge
  cannot tell a conversation from a broadcast: on UA KOLAB the heaviest pair
  exchanges 249 replies at a balance of 0.08 — one person answering another who
  almost never answers back — while a 9-and-9 pair at balance 1.00 is a genuine
  correspondence. Ranked on the total the second is invisible; ranked on the
  smaller direction it comes fourth.
- **`tenure` is measured against the archive's last day, not today.** An export
  is a fixed document, and reading the same file a year later must not silently
  reclassify everyone in it as gone.
- **`retention` keeps silent months on the axis.** Closing a six-month gap into
  a single step reports the first month after it as ordinary retention.
- **It costs 0.16 s and 27 KB** on a 333,582-message archive: one extra pass
  over the replies.

## Verification

**`save.bat baseline` is the load-bearing check.** It runs the program over both
real archives and compares the whole report and the whole stats dump, byte for
byte, against a recording. Three legs: `ua-kolab`, `krgm`, and `krgm-notes`,
which passes the 42 hand-written notes so the annotation layer's markup — the
rail, the markers, the cards, the coverage band, the "matches nobody" list — is
covered too.

| corpus | messages | topics |
|---|---:|---|
| `N:\telegram export\UA KOLAB TELEGRAM` | 6,643 | 4 |
| `J:\temp pureraw\KRGM*` | 333,582 | 10 |

Re-record with `save.bat baseline record` **only** for a change that is meant to
alter the output, and read the diff first.

**Neither corpus goes into git**, and neither does the baseline: both are
verbatim chat history from real people. The committed fixtures
(`tga-report/tests/fixture.stats.json`, `tests/synthetic/`) are hand-written and
name nobody, which is why they can be committed and run on a fresh clone with no
drives. They catch drift; they do not claim correctness.

`.gitattributes` forces `-text` on every golden and on the two asset files,
because a CRLF checkout would fail every byte-for-byte compare; `eol=crlf` on
`*.bat`, because cmd's `goto` cannot find a label in an LF-only batch file; and
`binary` on `*.ttf`, because a converted TTF is a font that no longer parses.

## Where this came from

It began as a port of a Python analyser, and for a while that original was the
oracle: the same two archives run through both, and the numbers *and the HTML*
diffed character for character. That was worth having — it checked 4,000 lines
of statistics against a working implementation rather than against reasoning —
and it is gone, along with the frozen second render path that kept it alive past
its natural expiry. `REFACTOR.md` records what that cost and what replaced it.

What survives from it is in the code as comments, and is worth keeping: the
rounding rule, the tie-breaks, the recorded force-layout coordinates, the two
notes layouts. Those are facts about behaviour, not about provenance.

## Still open

- **The notes still only cover one month of one topic of one archive.** The
  machinery is finished and proven; what is missing is writing more of them.
  `tga <folder> --digest` produces what a model reads, `analysis/EVENTS.md`
  tells it the shape, and `--from-stats ... --notes ...` re-renders in a second
  without touching the export.
- **The language handshake.** The exporter asks the user which language the
  export is in; the analyser should follow what the export says, which means the
  answer has to be recorded inside the export folder. It cannot go in
  `result.json` — that file is byte-exact with Telegram Desktop's own output —
  so it goes in a sidecar at the export root. A Desktop export has no such
  sidecar and never will, so a missing file is the normal case: fall back to
  English and say nothing. The report is already built with its strings behind
  one lookup, so wiring it in stays mechanical. The exporter's half is not this
  repo's to do.
- **`reference/`** still holds the recorded output of the deleted Python
  harness. Nothing reads it; it can be deleted whenever you like.
- No `origin` remote; `save.bat push` says so and stops.
