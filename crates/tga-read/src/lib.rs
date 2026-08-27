//! Load a finished export off disk into a model the metrics can walk.
//!
//! A port of `analyser/read.py` from the Python original at
//! `C:\Users\Kosta\Projekti\telegram`. The only input is a folder.
//! `result.json` files are found wherever they sit — that app writes one per
//! topic directly under the export root, while Telegram Desktop writes
//! `chats/chat_<id>/result.json` — so both layouts load without a flag.
//!
//! **Two decisions are made once, here, and every metric inherits them.**
//!
//! *Time is the export's local wall clock.* Desktop writes `date` as a naive
//! local timestamp and `date_unixtime` as UTC epoch seconds. Anything with a
//! calendar or a clock face on it — hour of day, weekday, messages per day —
//! reads [`Msg::when`], because "who posts at 3am" is a question about the
//! clock on the wall, not about UTC. Anything measuring a *duration* reads
//! [`Msg::unix`], which stays monotonic across a DST change where `when` does
//! not.
//!
//! *A sender is a typed peer key, never a name.* `from_id` is stable across
//! renames and `from` is only what that person was called at the time. Both
//! are kept; deciding what to do with them belongs to identity, not here.
//!
//! Nothing here reaches the network.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde_json::Value;

pub mod text;
pub use text::{domain_of, emoji_runs};

/// What Desktop writes into `file` for a document it declined to download.
const SKIPPED_PREFIX: &str = "(File exceeds";

// ---------------------------------------------------------------------------
// the model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: String,
    pub count: i64,
    /// Peer keys of the reactors Telegram actually named — never the whole
    /// list. It caps at three per *message* unless the export asked for the
    /// full one, and people who react anonymously are in it at no setting.
    pub named: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Msg {
    pub id: i64,
    pub topic: usize,
    pub when: NaiveDateTime,
    pub unix: i64,
    pub service: bool,
    pub action: String,
    pub sender: String,
    pub name: String,
    pub text: String,
    pub chars: usize,
    pub words: usize,
    pub reply_to: Option<i64>,
    pub forward_from: String,
    pub forward_id: String,
    pub media: String,
    pub file_size: i64,
    pub media_saved: bool,
    pub sticker_emoji: String,
    pub emoji: Vec<String>,
    pub domains: Vec<String>,
    pub mentions: Vec<String>,
    pub hashtags: Vec<String>,
    pub reactions: Vec<Reaction>,
    pub edited: bool,
    pub members: Vec<String>,
    pub grouped: Option<i64>,
}

impl Msg {
    pub fn reaction_total(&self) -> i64 {
        self.reactions.iter().map(|r| r.count).sum()
    }
}

#[derive(Debug, Clone)]
pub struct Member {
    pub key: String,
    pub name: String,
    pub role: String,
    pub joined: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct Topic {
    pub index: usize,
    pub name: String,
    pub folder: PathBuf,
    pub chat_id: Option<i64>,
    pub chat_type: String,
    pub created: Option<NaiveDateTime>,
    pub messages: usize,
    /// The topic's root message id. Every top-level message in a forum topic
    /// is sent as a reply to it — see [`one`].
    pub root: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Export {
    pub root: PathBuf,
    pub name: String,
    pub topics: Vec<Topic>,
    pub msgs: Vec<Msg>,
    pub roster: Vec<Member>,
    /// `None` when the export wrote no roster at all, which is a different
    /// thing from a roster it knows is short.
    pub roster_complete: Option<bool>,
    pub members_count: Option<i64>,
}

impl Export {
    /// Message id -> index into `msgs`. Ids are chat-global, so this spans
    /// topics.
    pub fn by_id(&self) -> std::collections::HashMap<i64, usize> {
        self.msgs
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id, i))
            .collect()
    }

    /// Real messages only — a service entry is not somebody talking.
    pub fn said(&self) -> impl Iterator<Item = &Msg> {
        self.msgs.iter().filter(|m| !m.service)
    }
}

// ---------------------------------------------------------------------------
// coercion
//
// Every one of these mirrors a Python idiom rather than a Rust one, because
// the shapes they meet are whatever Desktop happened to write. Where the
// Python reads `str(x or "")`, the truthiness matters: 0, false and "" all
// collapse to the empty string, and a port that only checks for null gets a
// literal "0" where the original has nothing.
// ---------------------------------------------------------------------------

/// `str(value or "")`.
fn as_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => {
            if n.as_f64() == Some(0.0) {
                String::new() // falsy
            } else {
                n.to_string()
            }
        }
        Some(Value::Array(a)) if a.is_empty() => String::new(),
        Some(other) => other.to_string(),
    }
}

/// `int(value)`, and 0 where Python would have raised.
fn as_int(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f.trunc() as i64))
            .unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn as_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(n)) => n.as_i64(),
        _ => None,
    }
}

/// `datetime.fromisoformat`, tolerant, `None` on anything unreadable.
fn as_dt(value: Option<&Value>) -> Option<NaiveDateTime> {
    let raw = match value {
        Some(Value::String(s)) if !s.is_empty() => s.as_str(),
        _ => return None,
    };
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(raw, fmt) {
            return Some(dt);
        }
    }
    None
}

/// Desktop's `text`: a bare string, or a list of strings and entity objects.
fn plain(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    Value::String(s) => out.push_str(s),
                    Value::Object(_) => out.push_str(&as_text(part.get("text"))),
                    _ => {}
                }
            }
            out
        }
        None | Some(Value::Null) => String::new(),
        Some(other) => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// finding the files
// ---------------------------------------------------------------------------

/// Every `result.json` at or under `root`, shallowest first.
///
/// Bounded, because an export folder holds tens of thousands of media files
/// and an unbounded walk pays for all of them to find at most a handful of
/// JSON files three levels down.
///
/// Order decides [`Topic::index`], which every downstream figure is keyed on,
/// so it is not incidental: shallowest first, and within a depth, by the
/// lowercased path. That is what Python's `sorted()` over `WindowsPath` does,
/// since `PureWindowsPath` compares case-insensitively.
pub fn find_results(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found: Vec<(usize, String, PathBuf)> = Vec::new();

    let direct = root.join("result.json");
    if direct.is_file() {
        found.push((0, String::new(), direct));
    }

    let mut frontier: Vec<PathBuf> = vec![root.to_path_buf()];
    for depth in 1..=max_depth {
        let mut next = Vec::new();
        for dir in &frontier {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                // glob's `*` skips dotfiles, and so does this.
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
                {
                    continue;
                }
                let candidate = path.join("result.json");
                if candidate.is_file() {
                    let sort_key = candidate.to_string_lossy().to_lowercase();
                    found.push((depth, sort_key, candidate));
                }
                next.push(path);
            }
        }
        frontier = next;
    }

    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut seen: HashSet<PathBuf> = HashSet::new();
    found
        .into_iter()
        .map(|(_, _, p)| p)
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// one message
// ---------------------------------------------------------------------------

fn entities(raw: &Value, out: &mut Msg) {
    if let Some(Value::Array(list)) = raw.get("text_entities") {
        for ent in list {
            if !ent.is_object() {
                continue;
            }
            let kind = as_text(ent.get("type"));
            let text = as_text(ent.get("text"));
            match kind.as_str() {
                "link" => out.domains.push(domain_of(&text)),
                "text_link" => out.domains.push(domain_of(&as_text(ent.get("href")))),
                "mention" => out
                    .mentions
                    .push(text.trim_start_matches('@').to_lowercase()),
                "mention_name" => {
                    let uid = ent.get("user_id");
                    let named = as_text(uid);
                    out.mentions.push(if named.is_empty() {
                        text.to_lowercase()
                    } else {
                        format!("user{named}")
                    });
                }
                "hashtag" => out
                    .hashtags
                    .push(text.trim_start_matches('#').to_lowercase()),
                _ => {}
            }
        }
    }
    if let Some(preview) = raw.get("link_preview") {
        if preview.is_object() {
            let url = as_text(preview.get("url"));
            if !url.is_empty() {
                let host = domain_of(&url);
                if !host.is_empty() && !out.domains.contains(&host) {
                    out.domains.push(host);
                }
            }
        }
    }
}

/// The shape of the attachment, normalised.
///
/// `media_type` is Telegram's own answer and wins when it is there. A photo
/// carries no `media_type` at all, so the `photo` key and then the MIME major
/// type stand in for it.
fn media_kind(raw: &Value) -> String {
    let declared = as_text(raw.get("media_type"));
    if !declared.is_empty() {
        return declared;
    }
    if !as_text(raw.get("photo")).is_empty() {
        return "photo".to_string();
    }
    let mime = as_text(raw.get("mime_type"));
    if mime.starts_with("image/") {
        return "photo".to_string();
    }
    if mime.starts_with("video/") {
        return "video_file".to_string();
    }
    if mime.starts_with("audio/") {
        return "audio_file".to_string();
    }
    if !as_text(raw.get("file")).is_empty() || !as_text(raw.get("file_name")).is_empty() {
        return "document".to_string();
    }
    String::new()
}

/// One message object into a [`Msg`], or `None` if it carries no date.
///
/// `root` is the topic's own id, and it is the reason this takes an extra
/// argument. **A forum topic is a thread**, so Telegram sends every top-level
/// message in one as a reply to the message that opened it. Read literally,
/// that makes 2,702 of the 3,518 "replies" in the UA KOLAB export answers to a
/// single message, gives one topic root 2,463 replies, and drags the median
/// response time out to days. Those are not replies and they are dropped here,
/// once, so no metric downstream has to know about it.
///
/// The General topic is exempt because it is not a real thread — its messages
/// carry no `reply_to_message_id` at all, which is the same reason
/// `messages.getReplies` returns nothing for it.
pub fn one(raw: &Value, topic: usize, root: Option<i64>) -> Option<Msg> {
    let when = as_dt(raw.get("date"))?;
    let service = as_text(raw.get("type")) == "service";
    let text = plain(raw.get("text"));

    let parent = match as_i64(raw.get("reply_to_message_id")) {
        Some(p) if Some(p) != root => Some(p),
        _ => None,
    };

    let unix = {
        let declared = as_int(raw.get("date_unixtime"));
        if declared != 0 {
            declared
        } else {
            // Python falls back to `when.timestamp()`, which reads a naive
            // datetime as *local* time. Desktop always writes date_unixtime,
            // so this path effectively never runs on a real export.
            use chrono::TimeZone;
            chrono::Local
                .from_local_datetime(&when)
                .earliest()
                .map(|dt| dt.timestamp())
                .unwrap_or(0)
        }
    };

    let mut out = Msg {
        id: as_int(raw.get("id")),
        topic,
        when,
        unix,
        service,
        action: as_text(raw.get("action")),
        sender: as_text(raw.get(if service { "actor_id" } else { "from_id" })),
        name: as_text(raw.get(if service { "actor" } else { "from" })),
        // `len()` on a Python str counts code points, not bytes.
        chars: text.chars().count(),
        // `str.split()` with no argument splits on whitespace runs and drops
        // the empties.
        words: text.split_whitespace().count(),
        emoji: emoji_runs(&text),
        text,
        reply_to: parent,
        forward_from: as_text(raw.get("forwarded_from")),
        forward_id: as_text(raw.get("forwarded_from_id")),
        media: media_kind(raw),
        file_size: 0,
        media_saved: false,
        sticker_emoji: as_text(raw.get("sticker_emoji")),
        domains: Vec::new(),
        mentions: Vec::new(),
        hashtags: Vec::new(),
        reactions: Vec::new(),
        edited: matches!(raw.get("edited"), Some(v) if !v.is_null() && v != &Value::Bool(false)),
        members: Vec::new(),
        grouped: as_i64(raw.get("grouped_id")),
    };

    entities(raw, &mut out);

    // `file_size or photo_file_size` — a zero is falsy, so it falls through.
    out.file_size = match as_int(raw.get("file_size")) {
        0 => as_int(raw.get("photo_file_size")),
        size => size,
    };
    let path = {
        let file = as_text(raw.get("file"));
        if file.is_empty() {
            as_text(raw.get("photo"))
        } else {
            file
        }
    };
    out.media_saved = !path.is_empty() && !path.starts_with(SKIPPED_PREFIX);

    if let Some(Value::Array(list)) = raw.get("reactions") {
        for entry in list {
            if !entry.is_object() {
                continue;
            }
            let emoji = {
                let e = as_text(entry.get("emoji"));
                if !e.is_empty() {
                    e
                } else {
                    let d = as_text(entry.get("document_id"));
                    if d.is_empty() {
                        "?".to_string()
                    } else {
                        d
                    }
                }
            };
            let named = match entry.get("recent") {
                Some(Value::Array(recent)) => recent
                    .iter()
                    .filter(|r| r.is_object())
                    .map(|r| as_text(r.get("from_id")))
                    .filter(|s| !s.is_empty())
                    .collect(),
                _ => Vec::new(),
            };
            out.reactions.push(Reaction {
                emoji,
                count: as_int(entry.get("count")),
                named,
            });
        }
    }

    if let Some(Value::Array(members)) = raw.get("members") {
        out.members = members.iter().map(|m| as_text(Some(m))).collect();
    }

    Some(out)
}

// ---------------------------------------------------------------------------
// the roster
// ---------------------------------------------------------------------------

fn roster(root: &Path) -> (Vec<Member>, Option<bool>) {
    let path = root.join("participants.json");
    if !path.is_file() {
        return (Vec::new(), None);
    }
    let body: Value = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(body) => body,
        None => return (Vec::new(), None),
    };

    let mut people = Vec::new();
    if let Some(Value::Array(list)) = body.get("members") {
        for entry in list {
            if !entry.is_object() {
                continue;
            }
            let key = as_text(entry.get("id"));
            if key.is_empty() {
                continue;
            }
            let role = match as_text(entry.get("role")) {
                r if r.is_empty() => "member".to_string(),
                r => r,
            };
            people.push(Member {
                key,
                name: as_text(entry.get("name")),
                role,
                joined: as_dt(entry.get("joined")),
            });
        }
    }

    // `complete` absent is not the same as `complete: false`. A roster the
    // export never wrote and a roster it knows is short look identical
    // downstream unless this distinction survives.
    let complete = match body.get("complete") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v != &Value::Bool(false) && v != &serde_json::json!(0)),
    };
    (people, complete)
}

// ---------------------------------------------------------------------------
// load
// ---------------------------------------------------------------------------

/// Called as `(done, total, label)` after each file is read.
///
/// Borrowed rather than owned so the caller can keep whatever it is writing
/// into — a CLI's stdout, or a window's progress bar — and `Option` rather
/// than a no-op default so a caller that wants nothing pays nothing.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &str);

/// Read every `result.json` under `root` into one [`Export`].
///
/// `progress` gives a window something to move while a large export is parsed:
/// the KRGM corpus is 333,582 messages, and every one of them is read before
/// the first figure can be computed.
pub fn load(root: &Path, mut progress: Option<Progress<'_>>) -> Result<Export> {
    let files = find_results(root, 3);
    anyhow::ensure!(!files.is_empty(), "No result.json under {}", root.display());

    let mut export = Export {
        root: root.to_path_buf(),
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..Default::default()
    };
    (export.roster, export.roster_complete) = roster(root);

    let total = files.len();
    for (index, path) in files.iter().enumerate() {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let body: Value =
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;

        let folder = path.parent().unwrap_or(root).to_path_buf();
        let mut topic = Topic {
            index,
            name: match as_text(body.get("name")) {
                n if n.is_empty() => folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                n => n,
            },
            folder,
            chat_id: as_i64(body.get("id")),
            chat_type: as_text(body.get("type")),
            created: as_dt(body.get("topic_created")),
            messages: 0,
            root: as_i64(body.get("topic_id")),
        };

        if export.members_count.is_none() {
            export.members_count = as_i64(body.get("members_count"));
        }

        if let Some(Value::Array(messages)) = body.get("messages") {
            for entry in messages {
                if !entry.is_object() {
                    continue;
                }
                if let Some(msg) = one(entry, index, topic.root) {
                    export.msgs.push(msg);
                    topic.messages += 1;
                }
            }
        }

        let label = topic.name.clone();
        export.topics.push(topic);
        if let Some(report) = progress.as_deref_mut() {
            report(index + 1, total, &label);
        }
    }

    export
        .msgs
        .sort_by(|a, b| a.unix.cmp(&b.unix).then_with(|| a.id.cmp(&b.id)));
    Ok(export)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(extra: Value) -> Value {
        let mut base = json!({
            "id": 10, "type": "message", "date": "2026-08-18T22:59:03",
            "date_unixtime": "1786000000", "from": "Someone", "from_id": "user1",
            "text": ""
        });
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        base
    }

    #[test]
    fn a_reply_to_the_topic_root_is_not_a_reply() {
        // The whole reason `one` takes a root: every top-level message in a
        // forum topic is sent as a reply to the message that opened it.
        let raw = msg(json!({"reply_to_message_id": 2}));
        assert_eq!(one(&raw, 0, Some(2)).unwrap().reply_to, None);
        assert_eq!(one(&raw, 0, Some(99)).unwrap().reply_to, Some(2));
        assert_eq!(one(&raw, 0, None).unwrap().reply_to, Some(2));
    }

    #[test]
    fn a_message_with_no_date_is_dropped() {
        let mut raw = msg(json!({}));
        raw["date"] = Value::Null;
        assert!(one(&raw, 0, None).is_none());
    }

    #[test]
    fn chars_counts_code_points_not_bytes() {
        // "ćaskanje" is 8 characters and 9 bytes. Getting this wrong inflates
        // every length figure for exactly the exports that are not English.
        let raw = msg(json!({"text": "ćaskanje"}));
        let out = one(&raw, 0, None).unwrap();
        assert_eq!(out.chars, 8);
        assert_eq!(out.words, 1);
    }

    #[test]
    fn a_skipped_file_is_not_a_saved_one() {
        let saved = one(&msg(json!({"file": "photos/x.jpg"})), 0, None).unwrap();
        assert!(saved.media_saved);
        let skipped = one(
            &msg(json!({"file": "(File exceeds maximum size. Change data exporting settings to download.)"})),
            0,
            None,
        )
        .unwrap();
        assert!(!skipped.media_saved);
    }

    #[test]
    fn a_service_entry_takes_its_sender_from_the_actor_fields() {
        let raw = msg(json!({
            "type": "service", "action": "invite_members",
            "actor": "Someone Else", "actor_id": "user2",
            "from": "wrong", "from_id": "user1"
        }));
        let out = one(&raw, 0, None).unwrap();
        assert!(out.service);
        assert_eq!(out.sender, "user2");
        assert_eq!(out.name, "Someone Else");
    }

    #[test]
    fn a_falsy_number_reads_as_empty_the_way_python_does() {
        // `str(x or "")` — 0 is falsy, so a zeroed id is no id, not "0".
        assert_eq!(as_text(Some(&json!(0))), "");
        assert_eq!(as_text(Some(&json!(12))), "12");
        assert_eq!(as_text(None), "");
    }

    #[test]
    fn an_absent_complete_flag_differs_from_a_false_one() {
        // A roster the export never wrote and one it knows is short must not
        // look the same downstream.
        assert_eq!(roster(Path::new("no-such-folder")).1, None);
    }
}
