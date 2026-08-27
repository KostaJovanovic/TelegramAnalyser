//! The port, against the implementation it was ported from.
//!
//! `tools/diff_stats.py` does the same comparison from the outside and prints
//! a readable report; this is the same check wired into `cargo test`, so a
//! regression fails the ordinary suite without anyone remembering to run a
//! Python script.
//!
//! It needs two things per corpus: the export folder, and a `stats.json`
//! dumped from the Python analyser with
//!
//! ```text
//! run_analyser.py "<export>" --stats reference/<name>.stats.json
//! ```
//!
//! **The skip is loud.** `TGA_REQUIRE_CORPUS=1` turns a missing corpus into a
//! failure. A corpus test that quietly passes having compared nothing is worse
//! than no test, because it reports as coverage.

use std::path::{Path, PathBuf};

use serde_json::Value;

struct Corpus {
    name: &'static str,
    export: &'static str,
    oracle: &'static str,
}

const CORPORA: &[Corpus] = &[
    Corpus {
        name: "ua-kolab",
        export: r"N:\telegram export\UA KOLAB TELEGRAM",
        oracle: "ua-kolab.stats.json",
    },
    Corpus {
        name: "krgm",
        export: r"J:\temp pureraw\KRGM TREĆI 2026.06.09",
        oracle: "krgm.stats.json",
    },
];

/// Fields allowed to differ, with the reason each was ruled out. Kept in step
/// with `tools/diff_stats.py`'s `EXCLUDED`.
///
/// * `graph/nodes[]/{x,y}` — 220 iterations of Fruchterman-Reingold amplify
///   the last bit of `hypot` into the third decimal. `graph::tests` matches
///   Python to 1e-12 on a graph too small to diverge, which is what tells this
///   apart from a mistranslation.
/// * `churn/events[]/names[]` — the export writes `"members": [null]` twice.
///   Python's `str(None)` names an arrival "None"; this reads it as no name.
///   The only deliberate divergence in the port, and it moves no number.
const EXCLUDED: &[&str] = &[
    "graph/nodes[]/x",
    "graph/nodes[]/y",
    "churn/events[]/names[]",
];

fn reference_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/tga-metrics.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("reference"))
}

fn required() -> bool {
    std::env::var("TGA_REQUIRE_CORPUS").as_deref() == Ok("1")
}

/// `/graph/nodes[3]/x` -> `graph/nodes[]/x`.
///
/// The leading separator has to go too: the walk starts at the document root
/// with an empty path, so every path it builds gains one, and an exclusion
/// list written without it silently matches nothing.
fn normalised(path: &str) -> String {
    let path = path.trim_start_matches('/');
    let mut out = String::with_capacity(path.len());
    let mut in_index = false;
    for ch in path.chars() {
        match ch {
            '[' => {
                in_index = true;
                out.push('[');
            }
            ']' => {
                in_index = false;
                out.push(']');
            }
            _ if in_index => {}
            _ => out.push(ch),
        }
    }
    out
}

fn compare(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    if out.len() > 40 || EXCLUDED.contains(&normalised(path).as_str()) {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                match (x.get(key), y.get(key)) {
                    (Some(l), Some(r)) => compare(l, r, &format!("{path}/{key}"), out),
                    (None, _) => out.push(format!("{path}/{key}: only in rust")),
                    (_, None) => out.push(format!("{path}/{key}: only in python")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (l, r)) in x.iter().zip(y).enumerate() {
                compare(l, r, &format!("{path}[{i}]"), out);
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            out.push(format!("{path}: length {} vs {}", x.len(), y.len()))
        }
        (Value::Number(x), Value::Number(y)) => {
            let (l, r) = (
                x.as_f64().unwrap_or(f64::NAN),
                y.as_f64().unwrap_or(f64::NAN),
            );
            // Both sides round in their own language before serialising, so an
            // exact match on a ratio would be luck rather than correctness.
            if (l - r).abs() > 1e-9 * l.abs().max(r.abs()).max(1.0) {
                out.push(format!("{path}: {l} vs {r}"));
            }
        }
        _ if a != b => out.push(format!("{path}: {a} vs {b}")),
        _ => {}
    }
}

fn check(corpus: &Corpus) {
    let export = PathBuf::from(corpus.export);
    let oracle = reference_dir().join(corpus.oracle);

    if !export.is_dir() || !oracle.is_file() {
        assert!(
            !required(),
            "TGA_REQUIRE_CORPUS=1 but {} is incomplete: export {} / oracle {}",
            corpus.name,
            if export.is_dir() { "ok" } else { "missing" },
            if oracle.is_file() { "ok" } else { "missing" },
        );
        eprintln!(
            "SKIP {}: needs {} and {}",
            corpus.name,
            export.display(),
            oracle.display()
        );
        return;
    }

    let python: Value =
        serde_json::from_str(&std::fs::read_to_string(&oracle).expect("read oracle"))
            .expect("parse oracle");
    let loaded = tga_read::load(&export, None).expect("load export");
    let (ours, _) = tga_metrics::analyse(&loaded);

    // A branch missing from either side is a failure, never a pass. Comparing
    // nothing and reporting no differences is how a port gets called finished.
    let mut branches: Vec<&String> = python
        .as_object()
        .expect("object")
        .keys()
        .chain(ours.as_object().expect("object").keys())
        .collect();
    branches.sort();
    branches.dedup();
    for branch in &branches {
        assert!(
            python.get(branch.as_str()).is_some() && ours.get(branch.as_str()).is_some(),
            "{}: branch {branch} is on one side only",
            corpus.name
        );
    }
    assert_eq!(
        branches.len(),
        tga_metrics::ALL_BRANCHES.len(),
        "{}: expected {} branches, found {}",
        corpus.name,
        tga_metrics::ALL_BRANCHES.len(),
        branches.len()
    );

    let mut diffs = Vec::new();
    compare(&python, &ours, "", &mut diffs);
    assert!(
        diffs.is_empty(),
        "{} differs from the Python analyser in {} place(s):\n  {}",
        corpus.name,
        diffs.len(),
        diffs.join("\n  ")
    );
}

#[test]
fn ua_kolab_matches_the_python_analyser() {
    check(&CORPORA[0]);
}

#[test]
fn krgm_matches_the_python_analyser() {
    check(&CORPORA[1]);
}
