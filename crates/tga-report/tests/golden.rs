//! The report against a committed copy of itself.
//!
//! **This catches drift. It does not claim correctness.** What it adds over
//! `save.bat baseline` — which compares whole reports on two real archives — is
//! that it runs on a fresh clone with no corpus and no drives, and that it names
//! the character where an edit changed the output.
//!
//! The fixture is **synthetic**, and that is forced rather than chosen:
//! `*.stats.json` is gitignored because a real dump is verbatim chat history
//! from real people, so a golden cut from a real archive could not be committed
//! at all. `fixture.stats.json` is hand-written to the shape
//! `tga_metrics::analyse` returns, and it carries the awkward cases on purpose
//! — a name with `&` and `<` in it, a person who never spoke, a two-day
//! silence, an alias list, a non-ASCII topic name, a superlative whose value is
//! a string rather than a count, and an export with no roster dates.
//!
//! **One value in it is deliberately not what the analyser would emit.** The
//! `dynamics.answer.minimum` floor is 3 here and 20 in real life: on a
//! 22-message fixture every hour is under 20, so the report would render only
//! the "not enough replies" branch and leave the chart, the median column and
//! the em-dash for a thin hour uncovered. The floor is a property of the
//! recorded dump rather than of the writer, which is what makes it fair to vary
//! — and two figures the fixture's seven-day archive genuinely cannot reach,
//! a dormant person and a returning one, are covered by `lib.rs`'s own tests
//! instead of by distorting it further.
//!
//! The fonts are **not** embedded in the golden. Three base64'd faces are
//! ~270 KB of noise in a committed file and they are already covered by
//! `assets::tests`; leaving them out keeps the golden readable in a diff, which
//! is the only reason to have one.
//!
//! To re-bless after a deliberate change:
//!
//! ```text
//! TGA_BLESS=1 cargo test -p tga-report --test golden
//! ```
//!
//! and read the resulting diff before committing it. A golden updated without
//! being read is a golden that records whatever the bug did.

use std::path::{Path, PathBuf};

use tga_notes::{Coverage, Event, Notes};
use tga_stats::Stats;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// A single annotation, so the event rail, the marker and the card all render.
///
/// Written here rather than read from a file: the events layer has its own
/// tests in `tga-notes`, and what this fixture is for is the *markup* an event
/// produces.
fn events() -> Vec<Event> {
    vec![
        Event {
            id: "migration".into(),
            start: chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            end: Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap()),
            title: "Moved off the old group".into(),
            summary: "A span, with citations and a confidence.".into(),
            kind: "milestone".into(),
            topic: Some(0),
            messages: vec![1, 2, 17],
            confidence: "high".into(),
            // The fields only the `_timeline` notes layout carries.
            time: "20:59".into(),
            weight: "major".into(),
            who: vec!["Ana".into(), "Nobody At All".into()],
            tags: vec!["osnivanje".into(), "kanali".into()],
        },
        Event {
            id: "quiet".into(),
            start: chrono::NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            end: None,
            title: "A moment, uncited & unsure".into(),
            summary: String::new(),
            // The loader defaults an absent kind to `milestone`, so an event
            // with no kind at all is a shape only a hand-built `Event`
            // reaches, and `rail` has a branch for it that nothing else covers.
            kind: String::new(),
            topic: None,
            messages: vec![],
            confidence: "low".into(),
            time: String::new(),
            weight: "minor".into(),
            who: vec![],
            tags: vec![],
        },
        // A second *named* kind, so the filter row is a real choice. With one
        // kind `kind_filters` renders nothing — a control that can only be
        // all-on or all-off is not a filter — and the golden would then pin an
        // absence rather than the feature.
        Event {
            id: "split".into(),
            start: chrono::NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(),
            end: None,
            title: "An argument".into(),
            summary: "A second kind, so the filters have something to choose between.".into(),
            kind: "drama".into(),
            topic: None,
            messages: vec![9],
            confidence: String::new(),
            time: "13:20".into(),
            weight: "medium".into(),
            who: vec!["Bob & Co <the second>".into()],
            tags: vec![],
        },
    ]
}

fn notes() -> Notes {
    Notes {
        events: events(),
        coverage: Some(Coverage {
            from: chrono::NaiveDate::from_ymd_opt(2025, 1, 1),
            to: chrono::NaiveDate::from_ymd_opt(2025, 1, 4),
            level: "turning points".into(),
            note: "Only the first four days were read.".into(),
        }),
        source: "events.json, written by a test".into(),
    }
}

fn stats() -> Stats {
    serde_json::from_str(
        &std::fs::read_to_string(here().join("fixture.stats.json"))
            .expect("the fixture is committed beside this test"),
    )
    .expect("the fixture still matches the shape `analyse` returns")
}

fn rendered() -> String {
    let stats = stats();
    tga_report::render(
        &stats,
        &tga_report::names_from_stats(&stats),
        &notes(),
        &tga_report::Options {
            embed_fonts: false,
            stamp: "5 September 2025".into(),
            ..Default::default()
        },
    )
}

#[test]
fn the_report_matches_the_committed_golden() {
    check_golden("golden.html", rendered());
}

#[test]
fn every_interactive_feature_is_drawn_into_the_document() {
    // The report renders every view server-side and lets JS only filter and pan
    // what is already there, so each of these is markup rather than a class
    // name. **Markup, not CSS.** The stylesheet names `.kind-filter` and
    // `.cov-read` whether or not anything uses them, so matching those alone
    // would let a feature that renders nothing pass on the strength of its own
    // rules.
    let html = rendered();
    for (what, needle) in [
        ("the search box", "<input class=\"find\""),
        ("the shared axis", "<div id=\"axis\" hidden"),
        ("the kind filters", "<div class=\"kinds\">"),
        ("a kind toggle", "<button class=\"kind-filter on\""),
        ("the coverage band", "<rect class=\"cov-read\""),
        ("the coverage line", "<p class=\"coverage\">"),
        ("the compact presence rows", "<path class=\"c"),
        ("the `who` line", "<p class=\"who\">"),
        ("the tag line", "<p class=\"tags\">"),
        ("a shape-coded marker", "<g class=\"ev k"),
        ("the between-people section", "<section id=\"between\">"),
        ("its nav entry", "href=\"#between\""),
    ] {
        assert!(html.contains(needle), "the report lost {what}");
    }
}

fn check_golden(name: &str, ours: String) {
    let path = here().join(name);

    if std::env::var("TGA_BLESS").as_deref() == Ok("1") {
        std::fs::write(&path, &ours).expect("write golden");
        eprintln!(
            "BLESSED {} ({} bytes) — read the diff",
            path.display(),
            ours.len()
        );
        return;
    }

    let golden = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no golden at {} — run `TGA_BLESS=1 cargo test -p tga-report --test golden`",
            path.display()
        )
    });

    if golden == ours {
        return;
    }
    let at = golden
        .char_indices()
        .zip(ours.char_indices())
        .find(|((_, a), (_, b))| a != b)
        .map(|((i, _), _)| i)
        .unwrap_or_else(|| golden.len().min(ours.len()));
    let window = |text: &str| {
        let from = at.saturating_sub(160).min(text.len());
        let to = (at + 200).min(text.len());
        text[from..to].to_string()
    };
    panic!(
        "the report drifted from the golden at character {at} \
         ({} bytes golden / {} bytes now)\n  golden: {}\n  now   : {}\n\n\
         If the change was deliberate: TGA_BLESS=1 cargo test -p tga-report --test golden",
        golden.len(),
        ours.len(),
        window(&golden),
        window(&ours)
    );
}

#[test]
fn the_golden_exercises_the_cases_it_was_built_for() {
    // A golden is only worth its bytes if it covers the awkward shapes. This
    // fails if the fixture is ever trimmed down to something tidy.
    let html = rendered();
    for (what, needle) in [
        ("an escaped name", "Bob &amp; Co &lt;the second&gt;"),
        ("an alias list", "class=\"alias\">was Bobby, B."),
        ("a non-member role", "class=\"role\">creator"),
        ("the overflow-free people table", "class=\"rank\">1<"),
        (
            "a silent member",
            "1 of the 3 people on the member list never posted",
        ),
        (
            "an incomplete roster",
            "member list in this export is incomplete",
        ),
        ("a non-ASCII topic", "ćaskanje"),
        ("a string-valued superlative", "class=\"big\">1.4 MB<"),
        ("a spanning event", "ev-span"),
        // `ev k1 conf-low`: the shape class sits between the two, so match the
        // part that is the actual claim.
        ("a low-confidence event", "conf-low"),
        ("a shape-coded marker", "class=\"ev k0"),
        ("the coverage prose", "Only the first four days were read."),
        (
            "a name that matches nobody",
            "class=\"unknown\">Nobody At All",
        ),
        ("a name that does match", "class=\"who\"><span>Ana</span>"),
        ("the weight", "class=\"w w-major\""),
        ("the time of day", "class=\"at\">20:59"),
        ("a tag", "class=\"tags\"><span>osnivanje"),
        ("an uncited event", "no messages cited"),
        ("an event with no summary", "No summary."),
        (
            "the hidden-node note",
            "The 1 least-connected people are left out",
        ),
        ("orphan replies", "1 replies point at a message"),
        ("an export with no roster dates", "carries no member list"),
        ("skipped media", "1 of those attachments were over the"),
        ("a quantile caption", "Five shades, one per fifth"),
        (
            "the streak",
            "Longest unbroken run of days posted on: Ana, 3 days",
        ),
        // -- the `dynamics` branch ------------------------------------------
        //
        // The fixture lowers `answer.minimum` to 3 on purpose. The real figure
        // is 20, which on a 22-message fixture would leave every hour under the
        // floor and render only the "not enough replies" branch — so the chart,
        // the median column and the em-dash for a thin hour would all go
        // uncovered. Lowering the floor is a property of the recorded dump, not
        // of the writer, which is what makes it a fair thing to vary here.
        (
            "a correspondent pair",
            "Ana &amp; Bob &amp; Co &lt;the second&gt;",
        ),
        ("both directions of a pair", "5 \u{2194} 3"),
        ("an hour that met the floor", "<td>2 min</td>"),
        ("an hour that did not", "<td>&#8212;</td>"),
        ("the chain-length buckets", " chains\""),
        // The heading rather than a bar: the fixture is a single month, so
        // `returning` is 0 by construction — nobody can come back in the first
        // month — and `columns` draws no rect for a zero. That absence is the
        // correct rendering and the chart is still there around it.
        ("the month-over-month chart", "<h3>Who came back</h3>"),
        ("nobody having gone quiet", "posted one in its last month"),
        (
            "dormancy measured against the archive",
            "7 Jan 2025, the last day",
        ),
    ] {
        assert!(html.contains(needle), "the golden lost {what}: {needle:?}");
    }
}

#[test]
fn the_golden_is_a_whole_self_contained_document() {
    let html = rendered();
    assert!(html.starts_with("<!doctype html>\n"));
    assert!(html.ends_with("</body></html>\n"));
    assert_eq!(
        html.matches("<section").count(),
        html.matches("</section>").count()
    );
    for scheme in ["http://", "https://", "//cdn"] {
        assert!(!html.contains(scheme), "the golden reaches for {scheme}");
    }
}
