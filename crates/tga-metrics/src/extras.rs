//! The bits that do not fit a chart: records, arrivals and departures, topics.
//!
//! Ported from `analyser/metrics/extras.py`.
//!
//! A superlative is a claim about one message, so each one carries the id and
//! date that back it. Nothing here reports a record without saying which
//! message it was — an unsourced "busiest day" is a number the reader cannot
//! check.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
use tga_read::{Export, Msg};

use crate::identity::People;
use crate::util::{round1, stamp_minutes, Counter};

/// Actions that add or remove somebody, as Desktop names them.
const JOIN_ACTIONS: &[&str] = &[
    "join_group_by_link",
    "invite_members",
    "invite_to_group_call",
];
const LEAVE_ACTIONS: &[&str] = &["remove_members"];

/// The small hours, for the night-owl figure. Stated in the report so the
/// reader can disagree with the boundary rather than guess at it.
const NIGHT: std::ops::Range<usize> = 0..5;
const MORNING: std::ops::Range<usize> = 5..9;

/// Truncating, like Desktop's own writer — 1,940,744 B is `1.8 MB`.
pub fn human_bytes(size: i64) -> String {
    let step = 1024.0f64;
    let mut value = size as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if value < step || unit == "GB" {
            if unit == "B" {
                return format!("{} B", value.trunc() as i64);
            }
            return format!("{:.1} {}", (value * 10.0).trunc() / 10.0, unit);
        }
        value /= step;
    }
    format!("{value:.1} GB")
}

fn snippet(msg: &Msg) -> String {
    const LIMIT: usize = 140;
    let text = msg.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return if msg.media.is_empty() {
            "(no text)".to_string()
        } else {
            format!("({})", msg.media.replace('_', " "))
        };
    }
    if text.chars().count() <= LIMIT {
        text
    } else {
        let head: String = text.chars().take(LIMIT - 1).collect();
        format!("{head}…")
    }
}

fn record(msg: &Msg, people: &People, value: Value, unit: &str) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    out.insert("id".into(), json!(msg.id));
    out.insert("topic".into(), json!(msg.topic));
    out.insert("date".into(), json!(stamp_minutes(&msg.when)));
    out.insert("who".into(), json!(people.name_of(&people.key_of(msg))));
    out.insert("text".into(), json!(snippet(msg)));
    out.insert("value".into(), value);
    out.insert("unit".into(), json!(unit));
    out
}

fn titled(title: &str, mut body: serde_json::Map<String, Value>) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("title".into(), json!(title));
    out.append(&mut body);
    Value::Object(out)
}

pub fn superlatives(export: &Export, people: &People, activity: &Value) -> Value {
    let msgs: Vec<&Msg> = export.said().collect();
    if msgs.is_empty() {
        return json!([]);
    }
    let mut out: Vec<Value> = Vec::new();

    // `max` on `(value, -id)`: the biggest, and on a tie the *lower* id.
    let best_by = |score: &dyn Fn(&Msg) -> i64| -> &Msg {
        msgs.iter()
            .copied()
            .max_by(|a, b| score(a).cmp(&score(b)).then_with(|| b.id.cmp(&a.id)))
            .expect("non-empty")
    };

    let reacted = best_by(&|m: &Msg| m.reaction_total());
    if reacted.reaction_total() > 0 {
        out.push(titled(
            "Most reacted to",
            record(
                reacted,
                people,
                json!(reacted.reaction_total()),
                "reactions",
            ),
        ));
    }

    let longest = best_by(&|m: &Msg| m.words as i64);
    if longest.words > 0 {
        out.push(titled(
            "Longest message",
            record(longest, people, json!(longest.words), "words"),
        ));
    }

    let biggest = best_by(&|m: &Msg| m.file_size);
    if biggest.file_size > 0 {
        out.push(titled(
            "Largest file",
            record(biggest, people, json!(human_bytes(biggest.file_size)), ""),
        ));
    }

    let mut replied: Counter<i64> = Counter::new();
    for msg in &msgs {
        if let Some(parent) = msg.reply_to {
            replied.bump(parent);
        }
    }
    if !replied.is_empty() {
        let (target_id, count) = replied.most_common(Some(1))[0];
        // Spans every message, service entries included — a reply can point at
        // one.
        if let Some(&at) = export.by_id().get(&target_id) {
            out.push(titled(
                "Most replied to",
                record(&export.msgs[at], people, json!(count), "replies"),
            ));
        }
    }

    if activity.get("empty") != Some(&json!(true)) {
        out.push(json!({
            "title": "Busiest day",
            "id": Value::Null, "topic": Value::Null,
            "date": activity["busiest_day"]["date"],
            "who": "", "text": "",
            "value": activity["busiest_day"]["messages"], "unit": "messages",
        }));
        let quiet = &activity["quietest"];
        if !quiet.is_null() {
            out.push(json!({
                "title": "Longest silence",
                "id": Value::Null, "topic": Value::Null,
                "date": format!("{} to {}",
                    quiet["from"].as_str().unwrap_or_default(),
                    quiet["to"].as_str().unwrap_or_default()),
                "who": "", "text": "",
                "value": quiet["days"], "unit": "days",
            }));
        }
    }

    Value::Array(out)
}

/// Per-person records, restricted to people with enough messages to mean it.
///
/// Without the floor every one of these is won by somebody who sent four
/// messages, all of them at 4am, and the report says something true about
/// nobody.
fn messages(row: &Value) -> i64 {
    row["messages"].as_i64().unwrap_or(0)
}

fn hour_share(row: &Value, hours: std::ops::Range<usize>) -> f64 {
    let counts = row["hours"].as_array().expect("hours");
    let sum: i64 = hours.map(|h| counts[h].as_i64().unwrap_or(0)).sum();
    sum as f64 / messages(row) as f64
}

pub fn awards(rows: &[Value], minimum: i64) -> Value {
    let eligible: Vec<&Value> = rows
        .iter()
        .filter(|r| r["messages"].as_i64().unwrap_or(0) >= minimum)
        .collect();
    if eligible.is_empty() {
        return json!([]);
    }

    // Plain functions rather than closures: a `Box<dyn Fn>` is `'static` by
    // default, and a closure over two locals cannot be.
    type Score = fn(&Value) -> f64;
    let picks: [(&str, &str, Score); 4] = [
        (
            "Night owl",
            "of their messages between midnight and 5am",
            |r| hour_share(r, NIGHT),
        ),
        ("Early bird", "of their messages between 5am and 9am", |r| {
            hour_share(r, MORNING)
        }),
        ("Most reacted to", "reactions per message they sent", |r| {
            r["reactions_received"].as_i64().unwrap_or(0) as f64 / messages(r) as f64
        }),
        ("Writes longest", "words per message on average", |r| {
            r["avg_words"].as_f64().unwrap_or(0.0)
        }),
    ];

    let mut out = Vec::new();
    for (title, note, score) in picks {
        let best = eligible
            .iter()
            .copied()
            .max_by(|a, b| {
                score(a)
                    .partial_cmp(&score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| messages(b).cmp(&messages(a)))
            })
            .expect("non-empty");
        let value = score(best);
        if value <= 0.0 {
            continue;
        }
        out.push(json!({
            "title": title,
            "name": best["name"],
            "key": best["key"],
            "value": if note.starts_with("of their") {
                format!("{:.0}%", value * 100.0)
            } else {
                format!("{value:.1}")
            },
            "note": note,
            "messages": messages(best),
        }));
    }
    Value::Array(out)
}

/// The longest run of consecutive days somebody posted on.
pub fn streaks(export: &Export, people: &People) -> Value {
    let mut order: Vec<String> = Vec::new();
    let mut per_person: HashMap<String, HashSet<NaiveDate>> = HashMap::new();
    for msg in export.said() {
        let key = people.key_of(msg);
        if key.is_empty() {
            continue;
        }
        per_person
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                HashSet::new()
            })
            .insert(msg.when.date());
    }

    let mut best_key = String::new();
    let mut best_run = 0usize;
    let mut best_end: Option<NaiveDate> = None;
    for key in &order {
        let mut days: Vec<NaiveDate> = per_person[key].iter().copied().collect();
        days.sort_unstable();
        // Note the shape: `run` is only ever *compared* inside the pairwise
        // walk, so somebody who posted on exactly one day never registers at
        // all. That is the Python behaviour and it is preserved.
        let mut run = 1usize;
        for pair in days.windows(2) {
            run = if (pair[1] - pair[0]).num_days() == 1 {
                run + 1
            } else {
                1
            };
            if run > best_run {
                best_key = key.clone();
                best_run = run;
                best_end = Some(pair[1]);
            }
        }
    }

    if best_key.is_empty() {
        return json!({});
    }
    json!({
        "name": people.name_of(&best_key),
        "days": best_run,
        "ended": best_end.map(|d| d.to_string()).unwrap_or_default(),
    })
}

/// Arrivals, departures, and how many people were actually talking.
///
/// Arrivals come from two places that disagree, and both are reported.
/// `participants.json` dates every *current* member's join, which is the
/// better source but says nothing about anyone who has since left. The service
/// messages in the history record joins and removals as they happened, but
/// only the ones Telegram announced.
pub fn churn(export: &Export, people: &People, activity: &Value) -> Value {
    let month_of = |year: i32, month: u32| format!("{year:04}-{month:02}");

    let mut joined: Counter<String> = Counter::new();
    for member in &export.roster {
        if let Some(when) = member.joined {
            joined.bump(month_of(when.year(), when.month()));
        }
    }

    let mut announced_join: Counter<String> = Counter::new();
    let mut announced_leave: Counter<String> = Counter::new();
    let mut events: Vec<Value> = Vec::new();
    for msg in &export.msgs {
        if !msg.service {
            continue;
        }
        let month = month_of(msg.when.year(), msg.when.month());
        let is_join = JOIN_ACTIONS.contains(&msg.action.as_str());
        let is_leave = !is_join && LEAVE_ACTIONS.contains(&msg.action.as_str());
        if !is_join && !is_leave {
            continue;
        }
        let count = if msg.members.is_empty() {
            1
        } else {
            msg.members.len()
        };
        if is_join {
            announced_join.add(month.clone(), count as i64);
        } else {
            announced_leave.add(month.clone(), count as i64);
        }
        events.push(json!({
            "date": stamp_minutes(&msg.when),
            "kind": if is_join { "join" } else { "leave" },
            "count": count,
            "who": if msg.name.is_empty() { people.name_of(&msg.sender) } else { msg.name.clone() },
            "names": msg.members,
        }));
    }

    let mut speaking: HashMap<String, HashSet<String>> = HashMap::new();
    for msg in export.said() {
        let key = people.key_of(msg);
        if key.is_empty() {
            continue;
        }
        speaking
            .entry(month_of(msg.when.year(), msg.when.month()))
            .or_default()
            .insert(key);
    }

    let mut months: Vec<String> = activity["per_month"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r[0].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    months.extend(joined.keys().cloned());
    months.extend(announced_join.keys().cloned());
    months.extend(speaking.keys().cloned());
    months.sort();
    months.dedup();

    json!({
        "months": months,
        "roster_joins": months.iter().map(|m| joined.get(m)).collect::<Vec<_>>(),
        "announced_joins": months.iter().map(|m| announced_join.get(m)).collect::<Vec<_>>(),
        "announced_leaves": months.iter().map(|m| announced_leave.get(m)).collect::<Vec<_>>(),
        "active": months.iter().map(|m| speaking.get(m).map_or(0, HashSet::len)).collect::<Vec<_>>(),
        "events": events,
        "roster_dated": joined.total(),
        "roster_size": export.roster.len(),
    })
}

/// Every metric that fits in a row, cut per topic.
pub fn topics(export: &Export, people: &People) -> Value {
    let mut out = Vec::new();
    for topic in &export.topics {
        let stream: Vec<&Msg> = export.said().filter(|m| m.topic == topic.index).collect();
        if stream.is_empty() {
            out.push(json!({
                "index": topic.index, "name": topic.name, "messages": 0,
                "voices": 0, "words": 0, "media": 0, "replies": 0,
                "reactions": 0, "first": "", "last": "", "top": "",
            }));
            continue;
        }
        let mut voices: Counter<String> = Counter::new();
        for msg in &stream {
            let key = people.key_of(msg);
            if !key.is_empty() {
                voices.bump(key);
            }
        }
        let words: i64 = stream.iter().map(|m| m.words as i64).sum();
        out.push(json!({
            "index": topic.index,
            "name": topic.name,
            "messages": stream.len(),
            "voices": voices.len(),
            "words": words,
            "avg_words": round1(words as f64 / stream.len() as f64),
            "media": stream.iter().filter(|m| !m.media.is_empty()).count(),
            "replies": stream.iter().filter(|m| m.reply_to.is_some()).count(),
            "reactions": stream.iter().map(|m| m.reaction_total()).sum::<i64>(),
            "first": stream[0].when.date().to_string(),
            "last": stream[stream.len() - 1].when.date().to_string(),
            "top": if voices.is_empty() {
                String::new()
            } else {
                people.name_of(&voices.most_common(Some(1))[0].0)
            },
        }));
    }
    Value::Array(out)
}
