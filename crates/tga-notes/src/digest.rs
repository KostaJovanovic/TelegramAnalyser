//! The input side: what you hand a model.
//!
//! Ported from `analyser/events.py`'s `write_digest` / `write_brief`.
//!
//! A [`Row`] is a message trimmed to what a reader needs in order to judge
//! *what happened*: who, when, in which topic, what they said, what it
//! answered, and how much the room reacted. Media becomes a marker rather than
//! a path, because a filename tells a model nothing and costs it tokens.
//!
//! Service messages belong in it — a join, a rename or a pin is often exactly
//! the moment worth marking.
//!
//! **`Row` is a plain struct rather than `&Msg`** so this crate does not depend
//! on `tga-read`; see the note in `Cargo.toml`. The caller does the mapping,
//! which is four lines and keeps `tga-report` free of the reader.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// How much of a message survives into the digest.
///
/// A model reading 333,582 messages pays for every character. Four hundred is
/// enough to tell what a message was about; a message longer than that is
/// almost always a copy-paste, and the id is in the row if anyone wants the
/// rest.
pub const MAX_CHARS: usize = 400;

/// One message, as the digest carries it.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub id: i64,
    /// Local wall clock, to the minute: `2025-09-05 14:30`.
    pub t: String,
    pub topic: usize,
    pub who: String,
    /// The action name, and only ever set on a service message.
    pub service: Option<String>,
    pub text: String,
    /// The message this answers.
    pub re: Option<i64>,
    pub media: String,
    pub reactions: Option<i64>,
}

/// `" ".join(text.split())`, then trimmed to [`MAX_CHARS`] with an ellipsis.
///
/// Whitespace is collapsed first because a pasted block arrives with its
/// newlines intact, and JSONL is one line per message by definition.
pub fn squeeze(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MAX_CHARS {
        return flat;
    }
    let head: String = flat.chars().take(MAX_CHARS - 1).collect();
    format!("{head}…")
}

/// `json.dumps(value, ensure_ascii=False)` for one string.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serialises")
}

/// Write the history as one JSONL file, one object per line, oldest first.
///
/// Key order is the Python's insertion order rather than sorted, and the
/// separators are `json.dumps`' defaults (`", "` and `": "`). Neither is
/// checked by a diff — nothing compares this file — but a digest that reads
/// differently between the two implementations is a needless difference in the
/// one artefact a human actually opens.
pub fn write_digest(rows: &[Row], out_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("digest.jsonl");
    let file = std::fs::File::create(&path)?;
    let mut out = BufWriter::new(file);

    for row in rows {
        let mut line = String::from("{");
        line.push_str(&format!("\"id\": {}", row.id));
        line.push_str(&format!(", \"t\": {}", quoted(&row.t)));
        line.push_str(&format!(", \"topic\": {}", row.topic));
        line.push_str(&format!(", \"who\": {}", quoted(&row.who)));
        if let Some(action) = &row.service {
            line.push_str(&format!(", \"service\": {}", quoted(action)));
        }
        if !row.text.is_empty() {
            line.push_str(&format!(", \"text\": {}", quoted(&row.text)));
        }
        if let Some(parent) = row.re {
            line.push_str(&format!(", \"re\": {parent}"));
        }
        if !row.media.is_empty() {
            line.push_str(&format!(", \"media\": {}", quoted(&row.media)));
        }
        if let Some(total) = row.reactions {
            line.push_str(&format!(", \"reactions\": {total}"));
        }
        line.push('}');
        // `newline="\n"` on the Python side, so no CRLF here either.
        out.write_all(line.as_bytes())?;
        out.write_all(b"\n")?;
    }
    // Buffered: without this the file can be left short, or empty, with no
    // error anywhere. Same lesson as `tgx-tg`'s `Output::close`.
    out.flush()?;
    Ok(path)
}

pub const BRIEF: &str = r#"# Mapping events onto the timeline

`digest.jsonl` is this export's whole history, one JSON object per line,
oldest first: `id`, `t` (local time), `topic`, `who`, `text`, and where they
apply `re` (the message it answers), `media`, `reactions` and `service`.

Read it and write `events.json` beside it. The analyser picks that file up on
its next run and pins each event to the report's timeline, where it can be
clicked to show the summary and jump to the messages it cites.

    {
      "version": 1,
      "source": "who or what wrote this",
      "events": [
        {
          "id": "migration",
          "date": "2025-12-14",
          "end": "2025-12-16",
          "kind": "milestone",
          "title": "Moved off the old group",
          "summary": "One or two sentences. What happened, and why it mattered.",
          "topic": 0,
          "messages": [1, 2, 17],
          "confidence": "high"
        }
      ]
    }

`date` and `title` are required; everything else is optional. `end` makes the
event a span rather than a moment. `kind` is free text -- the report groups by
whatever values it finds -- though `milestone`, `decision`, `conflict`,
`action`, `arrival` and `topic` are the ones it has names for.

Three rules that keep the timeline worth reading:

- **Cite.** Every event lists the message ids it rests on. An event with no
  `messages` is an assertion the reader cannot check, and the report marks it
  as one.
- **Mark what changed, not what was said.** A busy week is already in the
  ribbon underneath. An event earns its place by naming a decision, a split, a
  departure or a turn the numbers cannot show.
- **Say when you are guessing.** `confidence` is `high`, `medium` or `low`,
  and low is a perfectly good answer -- it is rendered differently, not hidden.
"#;

pub fn write_brief(out_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("EVENTS.md");
    std::fs::write(&path, BRIEF)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_collapsed_so_one_message_is_one_line() {
        assert_eq!(squeeze("a\n\n  b\tc  "), "a b c");
    }

    #[test]
    fn a_long_message_is_cut_to_the_budget_by_characters() {
        // By characters, not bytes: "ć" is two bytes and slicing on bytes
        // would split it and produce invalid UTF-8 in the middle of a JSON
        // string.
        let text = "ć".repeat(MAX_CHARS * 2);
        let cut = squeeze(&text);
        assert_eq!(cut.chars().count(), MAX_CHARS);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn a_message_exactly_at_the_budget_is_left_alone() {
        let text = "x".repeat(MAX_CHARS);
        assert_eq!(squeeze(&text), text);
    }

    #[test]
    fn optional_keys_are_absent_rather_than_null() {
        let dir = std::env::temp_dir().join("tga-notes-digest-test");
        let _ = std::fs::remove_dir_all(&dir);
        let rows = vec![
            Row {
                id: 1,
                t: "2025-09-05 14:30".into(),
                topic: 0,
                who: "Ana".into(),
                text: "hello".into(),
                ..Default::default()
            },
            Row {
                id: 2,
                t: "2025-09-05 14:31".into(),
                topic: 0,
                who: "Bob".into(),
                service: Some("invite_members".into()),
                re: Some(1),
                media: "photo".into(),
                reactions: Some(3),
                ..Default::default()
            },
        ];
        let path = write_digest(&rows, &dir).expect("write");
        let body = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = body.lines().collect();

        assert_eq!(
            lines[0],
            r#"{"id": 1, "t": "2025-09-05 14:30", "topic": 0, "who": "Ana", "text": "hello"}"#
        );
        assert!(!lines[0].contains("service"));
        assert!(!lines[0].contains("media"));
        assert!(lines[1].contains(r#""service": "invite_members""#));
        assert!(lines[1].contains(r#""re": 1"#));
        assert!(!lines[1].contains("text"), "no text, so no text key");
        assert!(body.ends_with('\n'));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_ascii_survives_unescaped() {
        // `ensure_ascii=False` on the Python side. A digest full of ć
        // escapes is one a reader cannot skim.
        assert_eq!(quoted("ćaskanje"), "\"ćaskanje\"");
    }
}
