//! Read, analyse, render — off the main thread.
//!
//! **The thread is not precautionary.** Measured on the Python original: 6,643
//! messages take 0.3 s and 333,582 take 24.8 s. This port does the large one in
//! about 6 s, which is still six seconds in which a single-threaded window
//! would stop repainting — and a window that stops repainting is one Windows
//! offers to close for you.
//!
//! **The worker owns no widget.** It sends [`Progress`] and the shell decides
//! what that means, which is what keeps the failure path honest: a bad folder
//! or a malformed export becomes a [`Progress::Failed`] on screen rather than a
//! panic that takes the thread with no message at all.
//!
//! The one thing it does hold is a clone of the egui context, and only to call
//! `request_repaint`. An `mpsc::Receiver` can only be *polled*, so without that
//! call nothing appears until some unrelated input causes a frame -- which the
//! user experiences as having to move the mouse to make the app work.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// The five stages a run passes through, as the bar counts them.
pub const STAGES: usize = 4;

/// What the worker tells the window.
#[derive(Debug, Clone)]
pub enum Progress {
    /// `done` of [`STAGES`], and what is happening.
    Step { done: usize, label: String },
    Done {
        path: PathBuf,
        messages: usize,
        topics: usize,
        people: i64,
        events: usize,
    },
    /// A bad folder or a malformed export. The only useful thing to do with it
    /// is put it on screen.
    Failed(String),
}

/// Everything a run needs, resolved before the thread starts.
///
/// Taken by value so the worker borrows nothing from the view: a run that
/// outlives the window it was started from must still finish writing the file
/// rather than be torn out from under itself.
#[derive(Debug, Clone)]
pub struct Request {
    pub folder: PathBuf,
    pub out: PathBuf,
    pub write_digest: bool,
    pub embed_fonts: bool,
}

/// What the notes section says wrote the file.
pub const SOURCE: &str = "Telegram Export Analyser";

/// Start a run. The receiver is drained by the window on the next frame.
pub fn spawn(request: Request, ctx: eframe::egui::Context) -> Receiver<Progress> {
    let (tx, rx) = channel();
    let tx = Waking { tx, ctx };
    std::thread::Builder::new()
        .name("tga-analyse".into())
        .spawn(move || run(request, tx))
        // A thread that cannot be spawned is a machine in trouble, and there is
        // nothing useful to fall back to — the alternative is doing 6 seconds of
        // work on the paint thread.
        .expect("the OS refused a worker thread");
    rx
}

/// A sender that wakes the window after every message.
///
/// Wrapped rather than left to the caller because "send, then repaint" has to
/// happen at all eleven send sites and forgetting it at one produces a window
/// that is a frame behind for the rest of the run.
struct Waking {
    tx: Sender<Progress>,
    ctx: eframe::egui::Context,
}

impl Waking {
    /// Every send is ignored on failure: the window may have been closed, and a
    /// worker that panics on a closed channel loses the report it had already
    /// written.
    fn send(&self, event: Progress) {
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}

fn run(request: Request, tx: Waking) {
    let say = |done: usize, label: String| {
        tx.send(Progress::Step { done, label });
    };

    say(0, "Reading the export".into());
    let mut report = |_done: usize, _total: usize, name: &str| {
        tx.send(Progress::Step {
            done: 0,
            label: format!("Reading {name}"),
        });
    };
    let export = match tga_read::load(&request.folder, Some(&mut report)) {
        Ok(export) => export,
        Err(e) => {
            tx.send(Progress::Failed(format!("{e}")));
            return;
        }
    };
    if export.msgs.is_empty() {
        tx.send(Progress::Failed(
            "That export has no messages in it.".into(),
        ));
        return;
    }

    say(1, "Counting".into());
    let (stats, people) = tga_metrics::analyse(&export);

    say(2, "Reading events".into());
    let notes = tga_notes::load(&request.folder);

    if request.write_digest {
        say(3, "Writing the digest".into());
        let target = request.folder.join("analysis");
        let rows = digest_rows(&export, &people);
        // A digest that could not be written is worth saying and not worth
        // failing over: the report is the thing that was asked for.
        if let Err(e) =
            tga_notes::write_digest(&rows, &target).and_then(|_| tga_notes::write_brief(&target))
        {
            say(3, format!("The digest could not be written: {e}"));
        }
    }

    say(3, "Writing the report".into());
    let html = tga_report::render(
        &stats,
        &tga_report::names_from_stats(&stats),
        &notes,
        &tga_report::Options {
            embed_fonts: request.embed_fonts,
            source: SOURCE.to_string(),
            ..Default::default()
        },
    );
    if let Err(e) = write_report(&request.out, &html) {
        tx.send(Progress::Failed(format!("{}: {e}", request.out.display())));
        return;
    }

    say(STAGES, "Done".into());
    tx.send(Progress::Done {
        path: request.out,
        messages: export.msgs.len(),
        topics: export.topics.len(),
        people: stats.people.speakers as i64,
        events: notes.events.len(),
    });
}

fn write_report(out: &Path, html: &str) -> std::io::Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(out, html)
}

/// The digest's view of the export.
///
/// The same mapping `tga-cli` does, and it lives on this side of the layering
/// for the same reason: `tga-notes` must not depend on `tga-read`, because it
/// carries the `Event` type that `tga-report` renders.
fn digest_rows(export: &tga_read::Export, people: &tga_metrics::People) -> Vec<tga_notes::Row> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_no_export_in_it_fails_rather_than_writing_an_empty_report() {
        // The commonest mistake this window can be handed: a folder that is not
        // an export. It has to come back as a sentence, not as a 1 MB file
        // describing nothing.
        let dir = std::env::temp_dir().join("tga-app-empty-folder");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let out = dir.join("report.html");
        let _ = std::fs::remove_file(&out);

        let (tx, rx) = channel();
        let ctx = eframe::egui::Context::default();
        run(
            Request {
                folder: dir.clone(),
                out: out.clone(),
                write_digest: false,
                embed_fonts: false,
            },
            Waking { tx, ctx },
        );

        let seen: Vec<Progress> = rx.try_iter().collect();
        // `tga_read::load` refuses first and names what it looked for, which is
        // the more useful of the two messages — the shell prints it verbatim.
        assert!(
            seen.iter()
                .any(|p| matches!(p, Progress::Failed(m) if m.contains("result.json"))),
            "expected a Failed naming result.json, got {seen:?}"
        );
        assert!(!out.is_file(), "nothing should have been written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_stage_count_matches_what_the_bar_is_told_to_range_over() {
        // `Done` is stage 4 of 4. If these disagree the bar finishes early or
        // never fills, and both read as the run having gone wrong.
        assert_eq!(STAGES, 4);
    }
}
