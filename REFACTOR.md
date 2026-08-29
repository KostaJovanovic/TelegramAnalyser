# REFACTOR.md

The plan for cutting this program loose from the Python original it was built to
copy, and cleaning up what that copying left behind.

Written in plain words on purpose. PLAN.md is the old record and stays as it is
until the work is done; this file is the proposal.

---

## What we agreed

1. **Before deleting anything, save today's output.** Run the current program on
   both archives, keep the results, and check every later step against them.
2. **Real Rust types for the statistics**, and keep the ability to save them to
   a file and rebuild a report from that file later.
3. **The window drops GPUI and uses egui instead** — plain buttons and text,
   close to the current look.
4. **The statistics and report code get cleaned up too.**

---

## Why the code feels bad right now

Five things, in the order they cost you:

**1. The statistics are an untyped JSON blob.**
`analyse()` returns `serde_json::Value`. Every field read looks like
`stats["people"]["rows"][0]["messages"].as_i64().unwrap_or(0)`. Rename a field
in the statistics and nothing fails to compile — the report just silently shows
zero. There are roughly 400 of these reads. This is the main problem.

**2. The report has two versions living in one file.**
`Options::classic` switches between the real report and a character-for-character
copy of the Python's HTML. About twenty `if classic` branches are scattered
through the rendering code, plus a whole set of light-mode colours that only the
copy uses, plus a bug that is reproduced on purpose so the comparison stays
clean. Every new feature has to be written twice, or wrapped so it doesn't leak.

**3. Three files are too big.**
`sections.rs` 1,927 lines. `charts.rs` 1,648. `report/lib.rs` 1,040 — and about
500 of those are CSS and JavaScript pasted inside Rust string literals, with no
syntax highlighting and no way to check them.

**4. Half the tests can't run.**
`oracle.rs` and `parity.rs` only work on your machine, with the two archives
plugged in and the Python code in a sibling folder. On any other machine they
skip. Four Python scripts in `tools/` exist only to feed them.

**5. The window carries 13.6 MB for one text box.**
`gpui-component` is in the dependency tree solely for the path field. GPUI itself
is a pre-1.0 library pinned to an exact version so a routine update can't break
the build.

---

## Step 0 — commit what's already there

The `dynamics` work is staged but not committed. It goes in as `v0.04` before
anything below starts, so there's a clean point to go back to.

## Step 1 — save today's output as the reference

Run the current program on both archives and keep:

```
baseline/ua-kolab.stats.json      the numbers
baseline/ua-kolab.report.html     the report
baseline/krgm.stats.json
baseline/krgm.report.html
```

Gitignored, like `reference/` — same reason, it's real people's conversation.

Then one command, `save.bat baseline`, re-runs the program and compares:

- **the report HTML, character for character.** Nothing in steps 2–4 is supposed
  to change one byte of it. If a byte moves, I did something I didn't intend.
- **the numbers, value by value.** Not character for character, because step 3
  changes how the file is laid out (see below) — it compares the parsed contents.

This replaces the Python as the thing that catches mistakes, and it is stricter
for this purpose: the Python could only ever confirm that Rust matched Python,
which is a different question from whether the refactor changed anything.

After step 5, the baseline files get re-recorded and committed to nothing — they
stay a local check.

## Step 2 — delete the Python connection

Nothing here changes the report a reader sees. It removes the second copy of it.

Deleted:

| what | where |
|---|---|
| the `--classic` flag | `tga-cli/src/main.rs` |
| `Options::classic` and every `if classic` branch | `tga-report`, ~20 places |
| the light colour set | `tga-report/src/palette.rs` |
| the deliberate `‘2` bug in the disclosure marker | `tga-report/src/lib.rs` |
| `golden-classic.html` and the test that leaks don't reach it | `tga-report/tests/` |
| `parity.rs`, `oracle.rs` | the two test files that need Python |
| `MASKED`, `EXCLUDED`, `ADDED`, `IMPLEMENTED`, `ALL_BRANCHES` | the lists of declared differences |
| `tools/diff_stats.py`, `diff_report.py`, `dump_python_stats.py`, `dump_python_report.py` | 4 files |
| `reference/` | the recorded Python output |
| `save.bat oracle`, `save.bat parity` | two menu entries |

Kept: the four ramp checks in `palette::tests`, but for the dark colours only.
`golden.html` and the synthetic four-message export stay — they run on any
machine with no archives, and they're the only test that covers the notes markup.

**Check:** both reports still identical to the baseline, character for character.

## Step 3 — real types for the statistics

This is the big one, and the one that fixes the daily annoyance.

**A new crate, `tga-stats`**, holding nothing but the data types and their
serde derives. `tga-metrics` fills them in; `tga-report` reads them. This keeps
the existing rule that the report crate doesn't depend on the reader or the
statistics code — it depends on the shape of the data, and now that shape has a
name.

```
tga-read  ──►  tga-metrics  ──►  tga-stats  ◄──  tga-report
                                (the types)
```

About 30 structs, one per branch and one per row type. Sketch:

```rust
pub struct Stats {
    pub export: ExportInfo,
    pub activity: Activity,
    pub people: People,
    pub content: Content,
    pub conversation: Conversation,
    pub graph: Graph,
    pub topics: Vec<Topic>,
    pub superlatives: Vec<Superlative>,
    pub awards: Vec<Award>,
    pub streak: Option<Streak>,
    pub churn: Churn,
    pub renamed: Vec<Renamed>,
    pub dynamics: Dynamics,
}

pub struct Activity {
    pub first: String,
    pub last: String,
    pub span_days: i64,
    pub active_days: i64,
    pub mean_per_active_day: f64,
    pub per_day: Vec<Count>,      // was [["2025-09-05", 41], ...]
    pub per_hour: [i64; 24],
    pub per_weekday: [i64; 7],
    pub hour_weekday: [[i64; 24]; 7],
    pub busiest_day: Busiest,
    pub quietest: Gap,
    pub empty: bool,
}
```

Three details worth deciding now rather than halfway through:

- **`per_hour` becomes `[i64; 24]` instead of `Vec<i64>`.** The count is fixed by
  the clock. Same for the 7 weekdays and the 7×24 grid. The chart code currently
  handles "what if there are 23" and there can't be.

- **`[label, count]` pairs become a named `Count { label, n }`.** There are nine
  of these branches — days, months, emoji, domains, hashtags, mentions,
  stickers, forward sources, message lengths. Today they're two-element arrays
  because Python wrote them that way. **This changes the layout of the saved
  stats file**: `["🙂", 41]` becomes `{"label": "🙂", "n": 41}`. Nothing outside
  this program reads that file, and the new form says what the numbers mean.
  Say so if you'd rather keep the old layout — it's possible, it just costs a
  bit of serde plumbing.

- **A superlative's value is sometimes a number and sometimes a text.** That gets
  a small two-case type rather than being papered over.

The report's `stats.rs` — the fifteen helper functions that dig values out of the
blob with a fallback for every one — is deleted. The fallbacks were guarding
against a shape mismatch that can no longer happen.

`--stats` writes `Stats` out; `--from-stats` reads it back. That keeps working,
and gains: a stats file from an older version now fails to load with a message
instead of rendering a report full of zeroes.

**Check:** both reports still identical to the baseline, character for character.
The numbers compare equal after parsing.

## Step 4 — break up the big files

No behaviour changes at all. Only where the code lives.

```
tga-report/
  assets/report.css        the ~350 lines currently inside a Rust string
  assets/report.js         the ~150 lines currently inside a Rust string
  src/lib.rs               the entry point and the page assembly. ~150 lines.
  src/sections/            one file per section:
      masthead.rs  timeline.rs  rhythm.rs  people.rs  conversation.rs
      dynamics.rs  content.rs   churn.rs   topics.rs  records.rs  notes.rs
  src/charts/              one file per chart:
      ribbon.rs  presence.rs  heatgrid.rs  calendar.rs  columns.rs
      bars.rs    network.rs   coverage.rs  markers.rs   scale.rs
  src/html.rs              escaping and the few shared tag helpers
  src/palette.rs           unchanged apart from step 2
```

The CSS and JS move into real `.css` and `.js` files pulled in at compile time
with `include_str!`. The bytes in the report are identical; the difference is
that you can now read and edit them with highlighting, and a stylesheet linter
can see them.

`tga-metrics/src/dynamics.rs` (745 lines, five unrelated figures) splits into
`dynamics/{pairs,answer,tenure,retention,depth}.rs`.

**Check:** both reports still identical to the baseline, character for character.
This is the step where that check earns its keep — it's a pure move, so any
difference is a mistake.

## Step 5 — the window, in egui

`tga-ui` and the current `tga-app` are deleted and replaced by one `tga-app`
built on **eframe/egui** with the OpenGL backend.

What the window does, unchanged: a path box you can paste into, a *Choose folder*
button, drag a folder anywhere onto it, two tick boxes, a progress bar, a status
line, and *Open report* when it's finished.

| now | after |
|---|---|
| `gpui` pinned to `=0.2.2` | `eframe`/`egui`, ordinary version range |
| `gpui-component` pinned to `=0.5.1`, 13.6 MB, for one text box | `egui::TextEdit` — caret, selection and clipboard included |
| `futures` + GPUI's executor for the progress channel | `std::sync::mpsc` + `ctx.request_repaint()` from the worker |
| gpui's `prompt_for_paths` | `rfd`, the native Windows folder dialog |
| `tga-ui`: hand-built rules, tick boxes, bar, buttons | egui's own, styled |
| ~15.2 MB executable | roughly 6 MB |

**Keeping the look.** egui out of the box is grey and rounded, so this is
deliberate work, not a default:

- the same colours, taken from the same place the report's stylesheet takes them,
  with the existing test that fails if the two drift apart;
- the same fonts — Geist Regular, Medium and Mono are already in the repo and
  egui loads them the same way;
- corners set to zero, hairline borders, flat fills, no shadows.

It will be close, not pixel-identical. The two things egui won't reproduce
exactly are the letter-spacing on the small capitals and the exact text
rendering weight.

**The window's logic gets tested.** The five existing tests — is the button live,
is the bar full, what does the status line say — currently live inside the GPUI
component. They move into a plain `state.rs` with no toolkit in it, so they keep
running and get easier to add to.

**Check:** by eye, side by side with a screenshot of the current window. There's
no automated check for a window and I'm not going to pretend there is.

## Step 6 — rewrite the documentation

`PLAN.md` describes a port with an expiry date. After this it describes a program.
`CLAUDE.md` loses the Python sections, the declared-difference lists, and the
"do not tidy the classic path" rule, and gains what actually matters now.

---

## What does *not* change

- The report a reader opens. Same design, same sections, same numbers.
- The reader (`tga-read`), the notes layer (`tga-notes`) and the digest. They
  were never shaped by the Python comparison and they're a reasonable size.
- One self-contained HTML file, no network, under 1 MB on the big archive.
- The two clocks, the peer-key rule, and the forum-topic rule. These are
  correctness, not leftovers.
- `save.bat` stays the entry point.

## Rough size of each step

| step | size | risk |
|---|---|---|
| 0 commit | minutes | none |
| 1 baseline | small | none |
| 2 delete Python | medium, mostly deletion | low — the check catches it |
| 3 real types | **large** | medium — 400 read sites, ~30 structs |
| 4 split files | medium, mechanical | low |
| 5 egui window | **large** | medium — new library, look is judged by eye |
| 6 docs | small | none |

Steps 2, 3 and 4 each end with both reports identical to the baseline. Step 5 is
independent of them and could be done first or last.

## Things I'd like you to cut or correct

- The `Count { label, n }` change in step 3 alters the saved stats file's layout.
  Fine, or keep the old two-element arrays?
- Step 4 splits `charts.rs` per chart. If you'd rather have fewer, bigger files,
  say so — I picked the split, not you.
- Step 5 says the look will be *close*. If exactly matching the current window
  matters, the webview option is the only one that can do it, and it's not too
  late to change.
- Anything in "what does not change" that you'd actually like changed while
  the code is open anyway.
