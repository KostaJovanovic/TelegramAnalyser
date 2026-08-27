//! The window: pick a folder, get a report.
//!
//! Ported from `analyser/window.py`'s `MainWindow`. The layout is a port; the
//! interaction rules are behaviour and survive the toolkit change unchanged.
//!
//! **A worker event repaints the window; the window never polls for one.**
//! [`Shell::start_pump`] awaits the job's channel on GPUI's *foreground*
//! executor and calls `cx.notify()`. This is the one rule from the exporter
//! worth carrying over without discovering it again: a `std::sync::mpsc`
//! receiver can only be polled, and polling it from `render` means nothing is
//! seen until some unrelated input causes a frame — which the user experiences
//! as having to move the mouse to make the app work.
//!
//! **One writer per fact.** The status line has one setter, and the run's
//! outcome is written by the pump alone. Both rules exist because the second
//! writer always wins a race nobody knew they had.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{
    div, px, App, Context, Entity, ExternalPaths, SharedString, Subscription, Task, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use tga_ui::components::{button, caps, eyebrow, leading, progress_bar, rule, thousands, tick_box};
use tga_ui::tokens::{rhythm, type_scale, Palette};

use crate::job::{self, Progress, Request, STAGES};
use crate::open;
use crate::theme;

pub const TITLE: &str = "Telegram Export Analyser";

const BLURB: &str = "Point this at a finished export. It reads the result.json files \
                     already on disk and writes one report.html beside them. Nothing \
                     is sent anywhere.";

pub struct Shell {
    palette: Palette,
    /// `dark` or `light`. Drives both the window and the report it writes —
    /// the window is the preview of what comes out, and a light report behind
    /// a dark window reads as two products.
    theme_name: &'static str,
    path: Entity<InputState>,
    write_digest: bool,
    embed_fonts: bool,

    status: SharedString,
    /// How far through the four stages, or `None` when nothing is running.
    stage: Option<usize>,
    running: bool,
    /// The last report written, and only if it is still openable.
    report: Option<PathBuf>,
    /// How many `result.json` files the current folder holds. `None` means the
    /// path is not a folder at all, which is a different thing from a folder
    /// with nothing in it.
    found: Option<usize>,

    _subs: Vec<Subscription>,
    /// The task draining the worker's channel. Dropped with the view, which is
    /// what stops the pump when the window closes.
    _pump: Option<Task<()>>,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = cx.new(|cx| InputState::new(window, cx).placeholder(r"C:\...\UA KOLAB"));

        let mut this = Self {
            palette: Palette::dark(),
            theme_name: "dark",
            path,
            write_digest: true,
            embed_fonts: true,
            status: "Choose an export folder.".into(),
            stage: None,
            running: false,
            report: None,
            found: None,
            _subs: Vec::new(),
            _pump: None,
        };

        // The folder is read from the field, not typed into a mirror of it: a
        // second copy of the same string is how a status line ends up
        // describing a folder other than the one in the box.
        this._subs.push(cx.subscribe_in(
            &this.path.clone(),
            window,
            |this, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.revalidate(cx);
                    cx.notify();
                }
            },
        ));
        this
    }

    // -- the folder --------------------------------------------------------

    /// What is in the field, with the quotes Explorer's "Copy as path" adds.
    fn folder(&self, cx: &App) -> Option<PathBuf> {
        let text = self
            .path
            .read(cx)
            .value()
            .as_ref()
            .trim()
            .trim_matches('"')
            .to_string();
        if text.is_empty() {
            None
        } else {
            Some(PathBuf::from(text))
        }
    }

    /// Re-count the `result.json` files under the field's folder.
    ///
    /// **This is the one check worth making before the run**, because it is the
    /// difference between "that is not an export" and six seconds of work ending
    /// in an error. The depth matches `tga_read::load`'s own, so a folder that
    /// counts here cannot come up empty there.
    fn revalidate(&mut self, cx: &mut Context<Self>) {
        self.report = None;
        let Some(folder) = self.folder(cx) else {
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
            .into()
        };
    }

    fn ready(&self) -> bool {
        !self.running && matches!(self.found, Some(n) if n > 0)
    }

    fn set_path(&mut self, folder: &std::path::Path, window: &mut Window, cx: &mut Context<Self>) {
        let text = folder.to_string_lossy().into_owned();
        self.path.update(cx, |state, cx| {
            if state.value().as_ref() != text {
                state.set_value(text, window, cx);
            }
        });
        self.revalidate(cx);
        cx.notify();
    }

    fn browse(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose an export folder".into()),
        });
        // `spawn_in`, not `spawn`: writing the chosen path back into the field
        // needs a `Window`, and only the window-flavoured async context carries
        // one.
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(chosen))) = paths.await else {
                return;
            };
            let Some(dir) = chosen.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| this.set_path(&dir, window, cx));
        })
        .detach();
    }

    /// A folder dropped onto the window.
    ///
    /// The first *directory* among the dropped paths wins, exactly as the
    /// Python's `dropEvent` does. Dropping `result.json` itself is a reasonable
    /// mistake and does nothing rather than half-working.
    fn dropped(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dir) = paths.paths().iter().find(|p| p.is_dir()) {
            self.set_path(&dir.clone(), window, cx);
        }
    }

    // -- the run -----------------------------------------------------------

    fn start(&mut self, _: &gpui::ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready() {
            return;
        }
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let request = Request {
            out: folder.join("report.html"),
            folder,
            theme: self.theme_name.to_string(),
            write_digest: self.write_digest,
            embed_fonts: self.embed_fonts,
        };
        self.running = true;
        self.report = None;
        self.stage = Some(0);
        self.status = "Reading the export".into();
        self.start_pump(job::spawn(request), cx);
        cx.notify();
    }

    /// Drain the worker's channel into this view, for the life of the run.
    ///
    /// The task awaits on GPUI's foreground executor, so an event arriving on
    /// the worker thread wakes it, it applies the batch, and it calls
    /// `cx.notify()`. Nothing here polls and nothing waits for a frame.
    ///
    /// Events are applied in **batches**: `tga_read::load` emits one per
    /// `result.json`, and a ten-topic export would otherwise queue ten repaints
    /// nobody sees.
    fn start_pump(
        &mut self,
        rx: futures::channel::mpsc::UnboundedReceiver<Progress>,
        cx: &mut Context<Self>,
    ) {
        use futures::StreamExt;
        self._pump = Some(cx.spawn(async move |this, cx| {
            let mut rx = rx;
            while let Some(first) = rx.next().await {
                let mut batch = vec![first];
                while let Ok(next) = rx.try_recv() {
                    batch.push(next);
                }
                let applied = this.update(cx, |this, cx| {
                    for event in batch {
                        this.apply(event);
                    }
                    cx.notify();
                });
                if applied.is_err() {
                    // The window is gone. Nothing left to notify — and the
                    // worker carries on to finish writing the file, which is
                    // the half that matters.
                    break;
                }
            }
        }));
    }

    fn apply(&mut self, event: Progress) {
        match event {
            Progress::Step { done, label } => {
                self.stage = Some(done);
                self.status = label.into();
            }
            Progress::Done {
                path,
                messages,
                topics,
                people,
                events,
            } => {
                self.status = done_status(&path, messages, topics, people, events).into();
                self.report = Some(path);
                self.stage = Some(STAGES);
                self.running = false;
            }
            Progress::Failed(message) => {
                self.status = message.into();
                self.stage = None;
                self.running = false;
            }
        }
    }

    fn open_report(&mut self, _: &gpui::ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.report.clone() else {
            return;
        };
        if let Err(e) = open::open(&path) {
            self.status = e.into();
            // The report is still on disk and still valid; only the handing-off
            // failed. Keeping the button live lets the user try again.
            cx.notify();
        }
    }

    // -- the options -------------------------------------------------------

    fn set_theme(&mut self, name: &'static str, cx: &mut Context<Self>) {
        self.theme_name = name;
        self.palette = Palette::named(name);
        theme::apply(&self.palette, cx);
        cx.notify();
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let running = self.running;
        let can_run = self.ready();
        let can_open = self.report.is_some() && !running;

        div()
            .id("shell")
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .font(tga_ui::fonts::sans())
            .px(px(34.0))
            .pt(px(30.0))
            .pb(px(26.0))
            // A folder dropped anywhere on the window, as the Python's
            // `setAcceptDrops(True)` does — the field is a small target and the
            // window is the obvious one.
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.dropped(paths, window, cx)
            }))
            .child(caps(TITLE, type_scale::MICRO, p.fg))
            .child(div().h(px(8.0)))
            .child(
                div()
                    .text_size(type_scale::BODY)
                    .line_height(leading(type_scale::BODY, rhythm::LINE_PROSE))
                    .text_color(p.muted)
                    .max_w(px(560.0))
                    .child(BLURB),
            )
            .child(div().h(px(22.0)))
            .child(rule(&p))
            .child(div().h(px(20.0)))
            // -- the folder ------------------------------------------------
            .child(eyebrow("Export folder", &p))
            .child(div().h(px(7.0)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.0))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .font(tga_ui::fonts::mono())
                            .child(Input::new(&self.path)),
                    )
                    .child(
                        div()
                            .id("choose")
                            .cursor_pointer()
                            .on_click(cx.listener(Self::browse))
                            .child(button("Choose", true, false, &p)),
                    ),
            )
            .child(div().h(px(20.0)))
            // -- the options -----------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(28.0))
                    .items_start()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(eyebrow("Theme", &p))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(0.0))
                                    .child(theme_cell("Dark", "dark", self.theme_name, &p, cx))
                                    .child(theme_cell("Light", "light", self.theme_name, &p, cx)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(eyebrow("Also", &p))
                            .child(option_row(
                                "digest",
                                "Write the digest an AI needs to map events",
                                self.write_digest,
                                !running,
                                &p,
                                cx.listener(|this, _, _, cx| {
                                    this.write_digest = !this.write_digest;
                                    cx.notify();
                                }),
                            ))
                            .child(option_row(
                                "fonts",
                                "Embed the fonts (keeps the report standalone)",
                                self.embed_fonts,
                                !running,
                                &p,
                                cx.listener(|this, _, _, cx| {
                                    this.embed_fonts = !this.embed_fonts;
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(div().flex_1())
            .child(rule(&p))
            .child(div().h(px(14.0)))
            // Always determinate. Every stage this window has is known and
            // counted, so there is nothing for `None` to mean here — and it
            // paints a 12% marker, which on a window that has not been asked to
            // do anything yet reads as a run already under way.
            .child(progress_bar(
                Some(self.stage.unwrap_or(0) as f32 / STAGES as f32),
                &p,
            ))
            .child(div().h(px(10.0)))
            // -- the footer ------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex_1()
                            .font(tga_ui::fonts::mono())
                            .text_size(type_scale::SMALL)
                            .text_color(p.muted)
                            .child(self.status.clone()),
                    )
                    .child(
                        div()
                            .id("open")
                            .when(can_open, |d| {
                                d.cursor_pointer().on_click(cx.listener(Self::open_report))
                            })
                            .child(button("Open report", can_open, false, &p)),
                    )
                    .child(
                        div()
                            .id("run")
                            .when(can_run, |d| {
                                d.cursor_pointer().on_click(cx.listener(Self::start))
                            })
                            .child(button("Analyse", can_run, true, &p)),
                    ),
            )
    }
}

/// What the status line says when a run finishes.
///
/// The same tally the CLI prints, plus the file it landed in — and the file
/// name, not the whole path, because the folder is already in the field two
/// inches above it. A free function so it can be checked without a window: the
/// plurals and the "and events, but only if there were any" are exactly the
/// sort of thing that reads fine and is wrong on one branch.
fn done_status(
    path: &std::path::Path,
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

/// One half of the theme switch. Two cells rather than a dropdown: there are
/// exactly two appearances, and a menu to choose between two things is a menu
/// too many.
fn theme_cell(
    label: &'static str,
    name: &'static str,
    current: &'static str,
    p: &Palette,
    cx: &mut Context<Shell>,
) -> gpui::Stateful<gpui::Div> {
    let on = current == name;
    div()
        .id(label)
        .cursor_pointer()
        .px(px(14.0))
        .py(px(7.0))
        .border_1()
        .border_color(if on { p.accent } else { p.hairline })
        .text_size(type_scale::BODY)
        .line_height(leading(type_scale::BODY, rhythm::LINE_TIGHT))
        .text_color(if on { p.accent } else { p.muted })
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| this.set_theme(name, cx)))
}

/// A tick box and its label, on one clickable row.
///
/// The whole row is the target, not the 12px box: a tick you have to aim at is
/// a tick people stop using.
fn option_row(
    id: &'static str,
    label: &'static str,
    ticked: bool,
    enabled: bool,
    p: &Palette,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.0))
        .when(enabled, |d| d.cursor_pointer().on_click(on_click))
        .child(tick_box(ticked, enabled, p))
        .child(
            div()
                .text_size(type_scale::BODY)
                .line_height(leading(type_scale::BODY, rhythm::LINE_TIGHT))
                .text_color(if enabled { p.fg } else { p.muted })
                .child(label),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The button-enabling rule, without a window.
    ///
    /// `ready()` reads two fields and nothing else, which is deliberate: the
    /// same question asked two ways is how a button ends up live during a run.
    fn ready(running: bool, found: Option<usize>) -> bool {
        !running && matches!(found, Some(n) if n > 0)
    }

    #[test]
    fn analyse_is_live_only_for_a_folder_with_an_export_in_it() {
        assert!(ready(false, Some(4)));
        assert!(!ready(false, Some(0)), "a folder with no result.json");
        assert!(!ready(false, None), "not a folder at all");
        assert!(!ready(true, Some(4)), "a run is already going");
    }

    /// The fraction the bar is given, from the stage the pump last set.
    fn fill(stage: Option<usize>) -> f32 {
        stage.unwrap_or(0) as f32 / STAGES as f32
    }

    #[test]
    fn the_bar_is_empty_at_rest_and_full_exactly_when_the_run_ends() {
        // `Done` sets the stage to STAGES, so the fraction is 1.0. Off by one
        // either way and a finished run reads as stuck at 75%.
        assert_eq!(fill(None), 0.0, "nothing has been asked for yet");
        assert_eq!(fill(Some(0)), 0.0);
        assert_eq!(fill(Some(STAGES)), 1.0);
    }

    #[test]
    fn a_finished_run_names_the_file_and_the_tally() {
        let status = done_status(
            std::path::Path::new(r"N:\telegram export\UA KOLAB\report.html"),
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
        let with = done_status(std::path::Path::new("report.html"), 10, 1, 2, 42);
        assert!(
            with.ends_with("1 topics, 42 events -> report.html"),
            "{with}"
        );
    }

    #[test]
    fn a_failed_run_empties_the_bar_rather_than_leaving_it_part_full() {
        // `Failed` clears the stage. A bar frozen at 50% under an error message
        // says the run is still going, which is the one thing it is not.
        assert_eq!(fill(None), 0.0);
    }
}
