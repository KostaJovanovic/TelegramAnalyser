//! Who talked, how much, and where.
//!
//! Ported from `analyser/metrics/people.py`.
//!
//! One row per identity, never per name — see [`crate::identity`]. Two counts
//! are kept apart on purpose and must not be added together:
//!
//! * **messages** is what somebody sent.
//! * **votes** is reactions they *gave*, and it is a floor rather than a
//!   total. Telegram names at most three reactors per message unless the
//!   export asked for the full list, and it never names anyone who reacted
//!   anonymously. The report says so wherever this number appears; presenting
//!   it as a total would be a wrong number that looks right.

use std::collections::HashMap;

use chrono::Timelike;
use serde_json::{json, Map, Value};
use tga_read::Export;

use crate::identity::People;
use crate::util::{round1, stamp_minutes};

#[derive(Default)]
struct Row {
    key: String,
    name: String,
    aliases: Vec<String>,
    role: String,
    joined: Option<String>,
    messages: i64,
    words: i64,
    chars: i64,
    media: i64,
    forwards: i64,
    replies: i64,
    edited: i64,
    reactions_received: i64,
    reacted_messages: i64,
    votes_given: i64,
    first: Option<String>,
    last: Option<String>,
    by_topic: HashMap<usize, i64>,
    hours: [i64; 24],
    silent: bool,
}

impl Row {
    fn into_value(self, total_msgs: usize) -> Value {
        let mut out = Map::new();
        out.insert("key".into(), json!(self.key));
        out.insert("name".into(), json!(self.name));
        out.insert("aliases".into(), json!(self.aliases));
        out.insert("role".into(), json!(self.role));
        out.insert("joined".into(), json!(self.joined));
        out.insert("messages".into(), json!(self.messages));
        out.insert("words".into(), json!(self.words));
        out.insert("chars".into(), json!(self.chars));
        out.insert("media".into(), json!(self.media));
        out.insert("forwards".into(), json!(self.forwards));
        out.insert("replies".into(), json!(self.replies));
        out.insert("edited".into(), json!(self.edited));
        out.insert("reactions_received".into(), json!(self.reactions_received));
        out.insert("reacted_messages".into(), json!(self.reacted_messages));
        out.insert("votes_given".into(), json!(self.votes_given));
        out.insert("first".into(), json!(self.first));
        out.insert("last".into(), json!(self.last));

        let mut by_topic = Map::new();
        for (index, n) in &self.by_topic {
            by_topic.insert(index.to_string(), json!(n));
        }
        out.insert("by_topic".into(), Value::Object(by_topic));
        out.insert("hours".into(), json!(self.hours));

        out.insert(
            "avg_words".into(),
            json!(if self.messages > 0 {
                round1(self.words as f64 / self.messages as f64)
            } else {
                0.0
            }),
        );
        out.insert(
            "share".into(),
            json!(if total_msgs > 0 {
                self.messages as f64 / total_msgs as f64
            } else {
                0.0
            }),
        );
        // Only ever set on a row that came from the roster, so it must stay
        // absent rather than false everywhere else — the Python dict has no
        // such key on a speaker and the diff would catch it.
        if self.silent {
            out.insert("silent".into(), json!(true));
        }
        Value::Object(out)
    }
}

pub fn compute(export: &Export, people: &People) -> Value {
    let msgs: Vec<_> = export.said().collect();

    // Insertion-ordered, because the final sort is stable and first-seen order
    // is what breaks a tie on (messages, name).
    let mut order: Vec<String> = Vec::new();
    let mut rows: HashMap<String, Row> = HashMap::new();

    macro_rules! row {
        ($key:expr) => {{
            let key: String = $key;
            rows.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                let person = people.get(&key);
                Row {
                    key: key.clone(),
                    name: person.label().to_string(),
                    aliases: person.aliases.clone(),
                    role: person.role.clone(),
                    joined: person
                        .joined
                        .map(|j| j.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    ..Default::default()
                }
            })
        }};
    }

    for msg in &msgs {
        let key = people.key_of(msg);
        if key.is_empty() {
            continue;
        }
        let entry = row!(key);
        entry.messages += 1;
        entry.words += msg.words as i64;
        entry.chars += msg.chars as i64;
        *entry.by_topic.entry(msg.topic).or_default() += 1;
        entry.hours[msg.when.hour() as usize] += 1;
        if !msg.media.is_empty() {
            entry.media += 1;
        }
        if !msg.forward_from.is_empty() || !msg.forward_id.is_empty() {
            entry.forwards += 1;
        }
        if msg.reply_to.is_some() {
            entry.replies += 1;
        }
        if msg.edited {
            entry.edited += 1;
        }
        if !msg.reactions.is_empty() {
            entry.reacted_messages += 1;
            entry.reactions_received += msg.reaction_total();
        }
        let stamp = stamp_minutes(&msg.when);
        if entry.first.is_none() {
            entry.first = Some(stamp.clone());
        }
        entry.last = Some(stamp);
    }

    let mut named_votes = 0i64;
    for msg in &msgs {
        for reaction in &msg.reactions {
            named_votes += reaction.named.len() as i64;
            for voter in &reaction.named {
                row!(voter.clone()).votes_given += 1;
            }
        }
    }

    let total_votes: i64 = msgs.iter().map(|m| m.reaction_total()).sum();

    // Somebody the roster knows about who never appears in the history. Worth
    // naming: in a group of 43 with 18 lurkers, "43 members" and "25 people
    // talked" are both true and only one of them describes the conversation.
    for person in people.iter() {
        if person.silent && !rows.contains_key(&person.key) {
            row!(person.key.clone()).silent = true;
        }
    }

    let mut ranked: Vec<Row> = order
        .iter()
        .map(|key| rows.remove(key).expect("row exists"))
        .collect();
    ranked.sort_by(|a, b| {
        b.messages
            .cmp(&a.messages)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let speakers: Vec<&Row> = ranked.iter().filter(|r| r.messages > 0).collect();
    let speaker_count = speakers.len();
    let silent_members = ranked.iter().filter(|r| r.messages == 0).count();
    // What share of the conversation the loudest few carry. A single number
    // for "is this a group or a broadcast".
    let top3: i64 = speakers.iter().take(3).map(|r| r.messages).sum();

    let total = msgs.len();
    let top3_share = if total > 0 {
        top3 as f64 / total as f64
    } else {
        0.0
    };

    json!({
        "rows": ranked.into_iter().map(|r| r.into_value(total)).collect::<Vec<_>>(),
        "speakers": speaker_count,
        "known_members": export.roster.len(),
        "roster_complete": export.roster_complete,
        "silent_members": silent_members,
        "total_messages": total,
        "top3_share": top3_share,
        "votes_named": named_votes,
        "votes_total": total_votes,
        "votes_anonymous": (total_votes - named_votes).max(0),
    })
}
