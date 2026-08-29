//! Every figure the report can show, computed in one pass over the export.
//!
//! [`analyse`] still returns a `serde_json::Value` rather than a typed struct.
//! That was the right shape while the dump had to line up field for field
//! against another program's, and it is the wrong one now: nothing checks a key
//! spelt wrong, so a renamed field reads as zero rather than failing to
//! compile. Step 3 of REFACTOR.md replaces it with named types in `tga-stats`.

pub mod activity;
pub mod content;
pub mod conversation;
pub mod dynamics;
pub mod extras;
pub mod graph;
pub mod identity;
pub mod people;
pub mod util;

use serde_json::{json, Value};
use tga_read::Export;

pub use identity::{People, Person};

/// Every branch [`analyse`] emits, in the order it builds them.
///
/// Named here rather than left implicit in the function body, so that "which
/// figures does this program compute" has an answer that can be read in one
/// place and asserted against.
pub const BRANCHES: &[&str] = &[
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
    "dynamics",
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
    out.insert("dynamics".into(), dynamics::compute(export, &who));

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
