//! The report, against the report it was ported from.
//!
//! The fidelity target for phase 2 is a *faithful port*, which is what buys
//! this: the Python analyser's own `report.html` is an oracle for the HTML
//! exactly as its `stats.json` was for the numbers. `tools/diff_report.py` does
//! the same comparison from outside and prints a readable report; this is the
//! same check wired into `cargo test`, so a regression fails the ordinary suite
//! without anyone remembering to run a Python script.
//!
//! It needs two things per corpus: a `stats.json` dumped from the Python
//! analyser, and its `report.html`, both under `reference/`:
//!
//! ```text
//! python tools/dump_python_stats.py  "<export>" reference/<name>.stats.json
//! python tools/dump_python_report.py "<export>" reference/<name>.html --stamp "<date>"
//! ```
//!
//! **The export folder is never opened here.** The whole point of `tga-report`
//! not depending on `tga-read` is that a recorded `stats.json` replays through
//! the writer on its own — so this leg runs with the drives unplugged, and it
//! is testing the writer rather than the reader, which `oracle.rs` already
//! covers.
//!
//! **The skip is loud.** `TGA_REQUIRE_CORPUS=1` turns a missing reference into
//! a failure. A parity test that quietly passes having compared nothing is
//! worse than no test, because it reports as coverage — the lesson three green
//! legs in `telegram_rust` cost.
//!
//! **The third leg never skips.** `tests/synthetic/` is a four-message export
//! written by hand, and its two recorded files are committed — it names nobody,
//! so unlike the corpora it is safe in git. It exists for one branch neither
//! real archive reaches: **both corpora have no `events.json`**, so until this
//! leg the entire annotation layer's markup — the rail, the markers, the cards,
//! the citations, the "skipped as malformed" note — had never been compared
//! against the implementation it was ported from.

use std::path::{Path, PathBuf};

use serde_json::Value;

struct Corpus {
    name: &'static str,
    stats: &'static str,
    report: &'static str,
}

const CORPORA: &[Corpus] = &[
    Corpus {
        name: "ua-kolab",
        stats: "ua-kolab.stats.json",
        report: "ua-kolab.html",
    },
    Corpus {
        name: "krgm",
        stats: "krgm.stats.json",
        report: "krgm.html",
    },
];

/// Everything the two implementations are allowed to differ on, with the reason
/// each was ruled out.
///
/// Printed on every failure, the way `tga-metrics/tests/oracle.rs` prints its
/// own list, because "the diff is noisy" and "the port is wrong" look identical
/// until somebody checks.
///
/// * **the interaction graph's coordinates** — the same divergence phase 1
///   declared for `graph/nodes[]/{x,y}`. 220 iterations of Fruchterman-Reingold
///   amplify the last bit of `hypot` into the third decimal; `graph::tests`
///   matches Python to 1e-12 on a graph too small to diverge, which is what
///   tells this apart from a mistranslation. The whole `<svg class="chart
///   network">` element is masked, since every number in it is derived from
///   those coordinates.
const MASKED: &[(&str, &str)] = &[(
    "the interaction graph's force-layout coordinates",
    "<svg class=\"chart network\"",
)];

fn reference_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/tga-report.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("reference"))
}

fn required() -> bool {
    std::env::var("TGA_REQUIRE_CORPUS").as_deref() == Ok("1")
}

/// Replace each masked element with its opening tag, so a difference *outside*
/// it still fails.
///
/// Masking by element rather than by attribute is deliberate: blanking only the
/// coordinates would leave the mask silently passing a network chart that had
/// stopped being drawn at all.
fn mask(html: &str) -> String {
    let mut out = html.to_string();
    for (_, opening) in MASKED {
        while let Some(start) = out.find(opening) {
            // An opening tag with no `</svg>` after it is malformed markup, not
            // a mask that has run out of work — leave it alone and let the diff
            // report it rather than looping forever on the same position.
            let Some(len) = out[start..].find("</svg>").map(|at| at + "</svg>".len()) else {
                break;
            };
            out.replace_range(start..start + len, "[MASKED]");
        }
    }
    out
}

/// The date the recorded Python report stamped into its Notes section.
///
/// Read back out rather than assumed, because the recording was made on
/// whatever day it was made and this test may run on any other. It is the only
/// value in the document that depends on when it was written.
fn stamp_of(html: &str) -> String {
    let after = html
        .split_once("Written by Telegram Export Analyser on ")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    after
        .split_once(" from ")
        .map(|(stamp, _)| stamp.to_string())
        .unwrap_or_default()
}

/// Where the two first differ, with enough either side to name the section.
fn first_difference(a: &str, b: &str) -> String {
    let at = a
        .char_indices()
        .zip(b.char_indices())
        .find(|((_, x), (_, y))| x != y)
        .map(|((i, _), _)| i)
        .unwrap_or_else(|| a.len().min(b.len()));
    let window = |text: &str| -> String {
        let from = text[..at.min(text.len())]
            .char_indices()
            .rev()
            .nth(160)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let to = text
            .char_indices()
            .skip_while(|(i, _)| *i < at)
            .nth(200)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        text[from..to].to_string()
    };
    format!(
        "at character {at} of {} (python) / {} (rust)\n  python: {}\n  rust  : {}",
        a.len(),
        b.len(),
        window(a),
        window(b)
    )
}

fn check(corpus: &Corpus) {
    let dir = reference_dir();
    let stats_path = dir.join(corpus.stats);
    let report_path = dir.join(corpus.report);

    if !stats_path.is_file() || !report_path.is_file() {
        assert!(
            !required(),
            "TGA_REQUIRE_CORPUS=1 but {} is incomplete: stats {} / report {}",
            corpus.name,
            if stats_path.is_file() {
                "ok"
            } else {
                "missing"
            },
            if report_path.is_file() {
                "ok"
            } else {
                "missing"
            },
        );
        eprintln!(
            "SKIP {}: needs {} and {} — see tools/dump_python_report.py",
            corpus.name,
            stats_path.display(),
            report_path.display()
        );
        return;
    }

    let stats: Value =
        serde_json::from_str(&std::fs::read_to_string(&stats_path).expect("read stats"))
            .expect("parse stats");
    let python = std::fs::read_to_string(&report_path).expect("read report");

    let ours = tga_report::render(
        &stats,
        &tga_report::names_from_stats(&stats),
        &tga_notes::Notes::default(),
        &tga_report::Options {
            stamp: stamp_of(&python),
            classic: true,
            ..Default::default()
        },
    );

    let (want, got) = (mask(&python), mask(&ours));
    // `assert!`, never `assert_eq!` — see the note in the annotated leg below.
    // On KRGM the two operands are 1.5 MB each.
    assert!(
        want == got,
        "{} differs from the Python report.\n{}\n\nDeclared divergences (masked, and \
         everything outside them is compared):\n{}",
        corpus.name,
        first_difference(&want, &got),
        MASKED
            .iter()
            .map(|(why, _)| format!("  - {why}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // The mask has to have masked something. Without this, a rename of the
    // network chart's class would turn the leg green by comparing two documents
    // that both had nothing where the graph should be.
    assert!(
        want.contains("[MASKED]"),
        "{}: nothing matched the mask, so it is no longer masking the graph",
        corpus.name
    );
}

#[test]
fn the_ua_kolab_report_matches_the_python_analyser() {
    check(&CORPORA[0]);
}

#[test]
fn the_krgm_report_matches_the_python_analyser() {
    check(&CORPORA[1]);
}

/// The committed four-message export, events file and all.
///
/// No skip and no environment: the fixture is in the repository, so this runs
/// on a fresh clone with no drives, no corpus and no Python. It is also the
/// only leg that renders an annotated report, because neither real archive has
/// an `events.json` in it.
#[test]
fn the_annotated_report_matches_the_python_analyser() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/synthetic");
    let stats: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("stats.json")).expect("read stats"))
            .expect("parse stats");
    let python = std::fs::read_to_string(dir.join("python.html")).expect("read report");

    // Through `tga_notes::load`, not a hand-built `Vec<Event>`: the Python read
    // the same `events.json` off the same folder, so loading it here is what
    // puts the loader under the oracle as well as the writer. The file carries
    // one deliberately malformed entry, which both sides must drop *and* count.
    let notes = tga_notes::load(&dir);
    assert_eq!(notes.events.len(), 1, "one usable event, one dropped");
    assert!(
        notes.source.contains("skipped as malformed"),
        "the dropped entry has to be reported, not silently swallowed: {}",
        notes.source
    );

    let ours = tga_report::render(
        &stats,
        &tga_report::names_from_stats(&stats),
        &notes,
        &tga_report::Options {
            embed_fonts: false,
            stamp: stamp_of(&python),
            classic: true,
            ..Default::default()
        },
    );

    let (want, got) = (mask(&python), mask(&ours));
    // `assert!`, never `assert_eq!`: the latter prints both operands in full,
    // which here is a hundred kilobytes of identical markup with the one
    // difference buried in it. `first_difference` is the whole point.
    assert!(
        want == got,
        "the annotated report differs from the Python analyser's.\n{}",
        first_difference(&want, &got)
    );
    // The things this leg exists for, named — so a fixture edited down to
    // something tidy fails here rather than passing having compared less.
    for needle in [
        "class=\"ev conf-high\"",
        "data-event=\"start\"",
        "id=\"event-start\"",
        "2 messages cited",
        "Events read from",
    ] {
        assert!(got.contains(needle), "the annotated leg lost {needle:?}");
    }
}

#[test]
fn the_stamp_is_read_back_out_of_the_recorded_report() {
    let html = "<p class=\"note\">Written by Telegram Export Analyser on 27 August 2026 \
                from <span class=\"num\">N:\\x</span>.</p>";
    assert_eq!(stamp_of(html), "27 August 2026");
    assert_eq!(stamp_of("no stamp here"), "");
}

#[test]
fn masking_removes_the_whole_element_and_not_just_its_numbers() {
    let html = "<p>before</p><svg class=\"chart network\" viewBox=\"0 0 1 1\">\
                <circle cx=\"1\"/></svg><p>after</p>";
    assert_eq!(mask(html), "<p>before</p>[MASKED]<p>after</p>");
}
