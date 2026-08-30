//! `TelegramAnalyser.exe <export folder> [options]` — the window's other half.
//!
//! Point it at a finished export and it writes one `report.html` beside it;
//! nothing here reaches the network.
//!
//! **This used to be a second executable, `tga.exe`.** One program that ships
//! as two files is two things to copy, two to sign and two to keep in step, for
//! a difference that is entirely in how it was launched. So the argv decides
//! instead: arguments mean do the work and print, none means open the window.
//! Every option below behaves exactly as it did when it had its own exe, which
//! is what lets `save.bat baseline` keep comparing byte for byte across the
//! change.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

const USAGE: &str = "\
Telegram Export Analyser

    TelegramAnalyser <export folder> [options]
    TelegramAnalyser --from-stats <stats.json> --out <report.html> [options]

    With no arguments at all, the window opens instead.

Options
    --out PATH        where to write the report (default: report.html in the
                      export folder)
    --digest          also write analysis/digest.jsonl and analysis/EVENTS.md,
                      which is what a model needs to map events onto the
                      timeline
    --no-fonts        do not embed the fonts; smaller file, needs Geist
                      installed to look right
    --stats PATH      write every computed figure as JSON
    --from-stats PATH re-render from a recorded --stats dump instead of
                      reading an export. No folder is needed and none is
                      opened: `tga-report` depends on neither the reader nor
                      the metrics, so a dump is the whole input. Use --notes to
                      annotate it.
    --notes PATH      read the events file from here rather than from beside
                      the export
    --stamp TEXT      the date the notes section prints, instead of today's.
                      The one value in the report that comes from the clock, so
                      pinning it is what makes two runs on different days
                      comparable byte for byte -- which is what `save.bat
                      baseline` rests on.
    --quiet           no per-file progress
";

/// Run as a command. `args` is argv with the program name already dropped, and
/// is never empty — an empty argv opens the window and never reaches here.
pub fn run(args: Vec<String>) -> Result<()> {
    if args[0] == "--help" || args[0] == "-h" {
        print!("{USAGE}");
        return Ok(());
    }

    // The folder is positional and optional: `--from-stats` needs no export,
    // and requiring a placeholder path to something that is never opened would
    // be a lie in the command line.
    let folder: Option<PathBuf> = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from);
    let flags_from = if folder.is_some() { 1 } else { 0 };
    let mut out: Option<PathBuf> = None;
    let mut from_stats: Option<PathBuf> = None;
    let mut notes_path: Option<PathBuf> = None;
    let mut digest = false;
    let mut embed_fonts = true;
    let mut stats_out: Option<PathBuf> = None;
    let mut quiet = false;
    let mut stamp: Option<String> = None;

    let mut rest = args[flags_from..].iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--out" => match rest.next() {
                Some(path) => out = Some(PathBuf::from(path)),
                None => bail!("--out needs a path"),
            },
            "--digest" => digest = true,
            "--no-fonts" => embed_fonts = false,
            "--stats" => match rest.next() {
                Some(path) => stats_out = Some(PathBuf::from(path)),
                None => bail!("--stats needs a path"),
            },
            "--from-stats" => match rest.next() {
                Some(path) => from_stats = Some(PathBuf::from(path)),
                None => bail!("--from-stats needs a path"),
            },
            "--notes" => match rest.next() {
                Some(path) => notes_path = Some(PathBuf::from(path)),
                None => bail!("--notes needs a path"),
            },
            "--stamp" => match rest.next() {
                Some(text) => stamp = Some(text.clone()),
                None => bail!("--stamp needs a date"),
            },
            "--quiet" => quiet = true,
            other => bail!("Unknown option: {other}\n\n{USAGE}"),
        }
    }

    // Built once and shared by both render paths, so `--from-stats` and a
    // straight read cannot drift apart in what they pass the writer.
    let options = tga_report::Options {
        embed_fonts,
        stamp: stamp.unwrap_or_else(tga_report::today_stamp),
        ..Default::default()
    };

    // The notes come from wherever they were named, or from beside the export.
    // A `--notes` path that is not there is an error rather than a shrug: the
    // ordinary "no annotations" case is not passing the flag at all, so naming
    // a file and getting silence would hide a typo.
    let notes = match (&notes_path, &folder) {
        (Some(path), _) => {
            if !path.is_file() {
                bail!("Not a file: {}", path.display());
            }
            tga_notes::load_file(path)
        }
        (None, Some(folder)) => tga_notes::load(folder),
        (None, None) => tga_notes::Notes::default(),
    };

    // -- re-render from a recorded dump, with no export on disk -------------
    if let Some(dump) = &from_stats {
        if stats_out.is_some() || digest {
            bail!(
                "--from-stats has no export to read, so --stats and --digest have nothing to write"
            );
        }
        let Some(out) = out else {
            bail!("--from-stats needs --out: there is no export folder to write beside");
        };
        let text = std::fs::read_to_string(dump)
            .map_err(|e| anyhow::anyhow!("{}: {e}", dump.display()))?;
        // A dump that no longer matches the shape now fails here, by name,
        // instead of rendering a report full of zeroes.
        let stats: tga_stats::Stats =
            serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", dump.display()))?;
        let html = tga_report::render(
            &stats,
            &tga_report::names_from_stats(&stats),
            &notes,
            &options,
        );
        write_utf8(&out, &html)?;
        println!(
            "{} people, {} topics{} (from {})",
            stats.people.speakers,
            stats.export.topics,
            if notes.events.is_empty() {
                String::new()
            } else {
                format!(", {} events", notes.events.len())
            },
            dump.display()
        );
        println!("{}", out.display());
        return Ok(());
    }

    let Some(folder) = folder else {
        bail!(
            "Nothing to read.

{USAGE}"
        );
    };
    if !folder.is_dir() {
        bail!("Not a folder: {}", folder.display());
    }
    let out = out.unwrap_or_else(|| folder.join("report.html"));

    let mut progress = |done: usize, total: usize, name: &str| {
        if !quiet {
            println!("  read {done}/{total}  {name}");
        }
    };
    let export = tga_read::load(&folder, Some(&mut progress))?;
    if export.msgs.is_empty() {
        bail!("That export has no messages in it.");
    }

    let (stats, people) = tga_metrics::analyse(&export);

    if let Some(path) = &stats_out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, tga_stats::write(&stats)?)?;
        println!("  {}", path.display());
    }

    if digest {
        let target = folder.join("analysis");
        let rows = digest_rows(&export, &people);
        println!("  {}", tga_notes::write_digest(&rows, &target)?.display());
        println!("  {}", tga_notes::write_brief(&target)?.display());
    }

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let html = tga_report::render(
        &stats,
        &tga_report::names_from_stats(&stats),
        &notes,
        &options,
    );
    write_utf8(&out, &html)?;

    println!(
        "{} messages, {} people, {} topics{}",
        thousands(export.msgs.len()),
        stats.people.speakers,
        export.topics.len(),
        if notes.events.is_empty() {
            String::new()
        } else {
            format!(", {} events", notes.events.len())
        }
    );
    println!("{}", out.display());
    Ok(())
}

/// The digest's view of the export.
///
/// This mapping lives here rather than in `tga-notes`, because that crate must
/// not depend on `tga-read` — it carries the `Event` type that `tga-report`
/// renders, and `tga-report` must not reach the reader. Four lines is the whole
/// price of the rule.
///
/// `pub(crate)` because the window writes digests too; it used to keep its own
/// identical copy, which is what two crates cost and one does not.
pub(crate) fn digest_rows(
    export: &tga_read::Export,
    people: &tga_metrics::People,
) -> Vec<tga_notes::Row> {
    export
        .msgs
        .iter()
        .map(|msg| tga_notes::Row {
            id: msg.id,
            t: msg.when.format("%Y-%m-%d %H:%M").to_string(),
            topic: msg.topic,
            who: if msg.sender.is_empty() && msg.name.is_empty() {
                String::new()
            } else {
                people.name_of(&people.key_of(msg))
            },
            service: if msg.service {
                Some(msg.action.clone())
            } else {
                None
            },
            text: tga_notes::digest::squeeze(&msg.text),
            re: msg.reply_to,
            media: msg.media.clone(),
            reactions: if msg.reactions.is_empty() {
                None
            } else {
                Some(msg.reaction_total())
            },
        })
        .collect()
}

/// Write UTF-8 with `\n` line endings, as the Python's `newline="\n"` does.
///
/// `std::fs::write` already does exactly this on every platform — the note is
/// here because the Python had to ask for it, and somebody comparing the two
/// will look for the equivalent.
fn write_utf8(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
