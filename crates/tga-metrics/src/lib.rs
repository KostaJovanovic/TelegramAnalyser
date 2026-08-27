//! Every figure the report can show, computed in one pass over the export.
//!
//! Ported from `analyser/metrics/`.
//!
//! [`analyse`] returns a `serde_json::Value` rather than a typed struct, and
//! that is deliberate rather than lazy. The Python original returns a plain
//! nested dict for two stated reasons — the report renders a table twin of
//! every chart and dicts make that mechanical, and the same structure dumps
//! to `stats.json` with no adapter. A third reason applies only to the port:
//! **that dump is the oracle.** Emitting the same shape keeps the diff against
//! Python's output a line-for-line comparison rather than an argument about
//! two schemas, which is what phase 1 is for.
//!
//! Once the numbers are proven, the report can be given typed input built from
//! these same functions. Not before.

pub mod activity;
pub mod content;
pub mod conversation;
pub mod extras;
pub mod graph;
pub mod identity;
pub mod people;
pub mod util;

use serde_json::{json, Value};
use tga_read::Export;

pub use identity::{People, Person};

/// Which branches of the Python dump this port currently produces.
///
/// Named here rather than inferred by the diff, so a branch cannot be
/// "passing" because nobody compared it. `tests/oracle.rs` fails if a branch
/// listed here is missing from either side, and reports the rest as outstanding
/// rather than silently ignoring them.
pub const IMPLEMENTED: &[&str] = ALL_BRANCHES;

/// Every branch the Python analyser emits, in the order `metrics/__init__.py`
/// builds them. The difference against [`IMPLEMENTED`] is the work left.
pub const ALL_BRANCHES: &[&str] = &[
    "export",
    "activity",
    "people",
    "content",
    "conversation",
    "graph",
    "topics",
    "superlatives",
    "awards",
    "streak",
    "churn",
    "renamed",
];

pub fn analyse(export: &Export) -> (Value, People) {
    let who = People::new(export);

    let mut out = serde_json::Map::new();
    out.insert(
        "export".into(),
        json!({
            "name": export.name,
            "root": util::path_string(&export.root),
            "topics": export.topics.len(),
            "messages": export.msgs.len(),
            "service_messages": export.msgs.iter().filter(|m| m.service).count(),
            "members_count": export.members_count,
            "roster_complete": export.roster_complete,
        }),
    );
    out.insert("activity".into(), activity::compute(export, &who));
    out.insert("people".into(), people::compute(export, &who));
    out.insert("content".into(), content::compute(export, &who));

    let talk = conversation::compute(export, &who);
    let weights: std::collections::HashMap<String, i64> = out["people"]["rows"]
        .as_array()
        .expect("rows is an array")
        .iter()
        .map(|r| {
            (
                r["key"].as_str().unwrap_or_default().to_string(),
                r["messages"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    let net = graph::build(
        talk["edges"].as_array().expect("edges"),
        talk["reaction_edges"].as_array().expect("reaction_edges"),
        &weights,
    );
    let acts = out["activity"].clone();
    out.insert("topics".into(), extras::topics(export, &who));
    out.insert(
        "superlatives".into(),
        extras::superlatives(export, &who, &acts),
    );
    out.insert(
        "awards".into(),
        extras::awards(out["people"]["rows"].as_array().expect("rows"), 40),
    );
    out.insert("streak".into(), extras::streaks(export, &who));
    out.insert("churn".into(), extras::churn(export, &who, &acts));
    out.insert("conversation".into(), talk);
    out.insert("graph".into(), net);

    let mut renamed: Vec<&Person> = who.renamed().collect();
    renamed.sort_by_key(|p| p.name.to_lowercase());
    out.insert(
        "renamed".into(),
        json!(renamed
            .iter()
            .map(|p| json!({ "name": p.name, "aliases": p.aliases }))
            .collect::<Vec<_>>()),
    );

    (Value::Object(out), who)
}
