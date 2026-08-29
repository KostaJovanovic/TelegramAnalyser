//! Everything the window decides, with no window in it.
//!
//! **This is the half worth testing**, and it is separated for that reason
//! rather than for tidiness: whether the Analyse button is live, how full the
//! bar is, and what the status line says on a finished run are all rules that
//! read fine and are wrong on one branch. None of them needs a toolkit to
//! check, and none of them changed when the toolkit did.
//!
//! **One writer per fact.** The status line has one setter and the run's
//! outcome is written by [`State::apply`] alone, because the second writer
//! always wins a race nobody knew they had.

use std::path::{Path, PathBuf};

use crate::job::{Progress, STAGES};

pub struct State {
    /// Exactly what is in the text field. The folder is read back out of it
    /// rather than kept in a second string: two copies of the same path is how
    /// a status line ends up describing a folder other than the one on screen.
    pub path: String,
    pub write_digest: bool,
    pub embed_fonts: bool,

    pub status: String,
    /// How far through the four stages, or `None` when nothing is running.
    pub stage: Option<usize>,
    pub running: bool,
    /// The last report written, and only if it is still openable.
    pub report: Option<PathBuf>,
    /// How many `result.json` files the current folder holds. `None` means the
    /// path is not a folder at all, which is a different thing from a folder
    /// with nothing in it.
    pub found: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        State {
            path: String::new(),
            write_digest: true,
            embed_fonts: true,
            status: "Choose an export folder.".into(),
            stage: None,
            running: false,
            report: None,
            found: None,
        }
    }
}

impl State {
    /// What is in the field, with the quotes Explorer's "Copy as path" adds.
    pub fn folder(&self) -> Option<PathBuf> {
        let text = self.path.trim().trim_matches('"');
        if text.is_empty() {
            None
        } else {
            Some(PathBuf::from(text))
        }
    }

    /// Re-count the `result.json` files under the field's folder.
    ///
    /// **This is the one check worth making before the run**, because it is the
    /// difference between "that is not an export" and six seconds of work
    /// ending in an error. The depth matches `tga_read::load`'s own, so a folder
    /// that counts here cannot come up empty there.
    pub fn revalidate(&mut self) {
        self.report = None;
        let Some(folder) = self.folder() else {
            self.found = None;
            self.status = "Choose an export folder.".into();
            return;
        };
        if !folder.is_dir() {
            self.found = None;
            self.status = "Not a folder.".into();
            return;
        }
        let files = tga_read::find_results(&folder, 3);
        self.found = Some(files.len());
        self.status = if files.is_empty() {
            "No result.json anywhere under that folder.".into()
        } else {
            format!(
                "{} result.json {} found.",
                files.len(),
                if files.len() == 1 { "file" } else { "files" }
            )
        };
    }

    pub fn set_path(&mut self, folder: &Path) {
        let text = folder.to_string_lossy().into_owned();
        if self.path != text {
            self.path = text;
        }
        self.revalidate();
    }

    /// Whether Analyse does anything.
    ///
    /// Two fields and nothing else, deliberately: the same question asked two
    /// ways is how a button ends up live during a run.
    pub fn ready(&self) -> bool {
        !self.running && matches!(self.found, Some(n) if n > 0)
    }

    /// How full the bar is.
    ///
    /// **Always determinate.** Every stage this window has is known and counted,
    /// so there is nothing for an indeterminate bar to mean — and a window that
    /// has been asked to do nothing yet would read as a run already under way.
    pub fn fill(&self) -> f32 {
        self.stage.unwrap_or(0) as f32 / STAGES as f32
    }

    pub fn apply(&mut self, event: Progress) {
        match event {
            Progress::Step { done, label } => {
                self.stage = Some(done);
                self.status = label;
            }
            Progress::Done {
                path,
                messages,
                topics,
                people,
                events,
            } => {
                self.status = done_status(&path, messages, topics, people, events);
                self.report = Some(path);
                self.stage = Some(STAGES);
                self.running = false;
            }
            Progress::Failed(message) => {
                self.status = message;
                self.stage = None;
                self.running = false;
            }
        }
    }
}

/// A count, grouped for reading, as the report groups its own.
///
/// The separator is **U+00A0**, not a space: a group separator that wraps puts
/// "6" at the end of one line and "643" at the start of the next.
pub fn thousands(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if negative {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(ch);
    }
    out
}

/// What the status line says when a run finishes.
///
/// The same tally the CLI prints, plus the file it landed in — and the file
/// name, not the whole path, because the folder is already in the field two
/// inches above it.
pub fn done_status(
    path: &Path,
    messages: usize,
    topics: usize,
    people: i64,
    events: usize,
) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    format!(
        "{} messages, {people} people, {topics} topics{} -> {name}",
        thousands(messages as i64),
        if events == 0 {
            String::new()
        } else {
            format!(", {events} events")
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(running: bool, found: Option<usize>) -> State {
        State {
            running,
            found,
            ..Default::default()
        }
    }

    #[test]
    fn analyse_is_live_only_for_a_folder_with_an_export_in_it() {
        assert!(with(false, Some(4)).ready());
        assert!(
            !with(false, Some(0)).ready(),
            "a folder with no result.json"
        );
        assert!(!with(false, None).ready(), "not a folder at all");
        assert!(!with(true, Some(4)).ready(), "a run is already going");
    }

    #[test]
    fn the_bar_is_empty_at_rest_and_full_exactly_when_the_run_ends() {
        // `Done` sets the stage to STAGES, so the fraction is 1.0. Off by one
        // either way and a finished run reads as stuck at 75%.
        let mut state = State::default();
        assert_eq!(state.fill(), 0.0, "nothing has been asked for yet");
        state.stage = Some(0);
        assert_eq!(state.fill(), 0.0);
        state.stage = Some(STAGES);
        assert_eq!(state.fill(), 1.0);
    }

    #[test]
    fn a_failed_run_empties_the_bar_rather_than_leaving_it_part_full() {
        // A bar frozen at 50% under an error message says the run is still
        // going, which is the one thing it is not.
        let mut state = State {
            running: true,
            stage: Some(2),
            ..Default::default()
        };
        state.apply(Progress::Failed("Not a folder.".into()));
        assert_eq!(state.fill(), 0.0);
        assert!(!state.running);
        assert_eq!(state.status, "Not a folder.");
    }

    #[test]
    fn a_finished_run_names_the_file_and_the_tally() {
        let status = done_status(
            Path::new(r"N:\telegram export\UA KOLAB\report.html"),
            6_643,
            4,
            45,
            0,
        );
        // The count is grouped with a non-breaking space, as the report's is.
        assert_eq!(
            status,
            "6\u{a0}643 messages, 45 people, 4 topics -> report.html"
        );
        assert!(!status.contains("events"), "no events file, so no mention");
    }

    #[test]
    fn events_are_mentioned_only_when_there_were_some() {
        // "0 events" on an export with no events file reads as a failure to
        // find them rather than as there being none to find.
        let with = done_status(Path::new("report.html"), 10, 1, 2, 42);
        assert!(
            with.ends_with("1 topics, 42 events -> report.html"),
            "{with}"
        );
    }

    #[test]
    fn a_path_pasted_out_of_explorer_keeps_its_quotes_off() {
        // "Copy as path" wraps the whole thing in double quotes, and a path with
        // them still attached is a folder that does not exist.
        let mut state = State {
            path: "  \"N:\\telegram export\\UA KOLAB\"  ".into(),
            ..Default::default()
        };
        assert_eq!(
            state.folder(),
            Some(PathBuf::from(r"N:\telegram export\UA KOLAB"))
        );
        state.path = "   ".into();
        assert_eq!(state.folder(), None, "whitespace is not a folder");
    }

    #[test]
    fn a_finished_report_is_forgotten_the_moment_the_folder_changes() {
        // Otherwise Open report opens the *previous* folder's file, which is a
        // real report about the wrong archive.
        let mut state = State {
            report: Some(PathBuf::from("old/report.html")),
            ..Default::default()
        };
        state.set_path(Path::new("nowhere-at-all"));
        assert!(state.report.is_none());
    }
}
