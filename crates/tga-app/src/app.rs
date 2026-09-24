//! The window: pick a folder, get a report.
//!
//! **A worker event repaints the window; the window never polls for one.** The
//! job hands back a `std::sync::mpsc::Receiver`, which can only be polled — so
//! the worker also holds a clone of the egui context and calls
//! `request_repaint()` on every message. Without that, nothing appears until
//! some unrelated input causes a frame, which the user experiences as having to
//! move the mouse to make the app work.
//!
//! Everything this window *decides* is in [`crate::state`]; what is here is
//! where things sit and what they look like.

use std::path::PathBuf;

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};

use crate::job::{self, Request};
use crate::open;
use crate::state::State;
use crate::theme::{self, size, Palette};

pub const TITLE: &str = "Telegram Export Analyser";

const BLURB: &str = "Point this at a finished export — a folder of result.json files, or \
                     the exporter's telegram.sqlite. It reads what is already on disk and \
                     writes one report.html beside it. Nothing is sent anywhere.";

pub struct Shell {
    state: State,
    /// Drained every frame. `None` between runs.
    progress: Option<std::sync::mpsc::Receiver<job::Progress>>,
}

impl Shell {
    pub fn new(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        Shell {
            state: State::default(),
            progress: None,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        if !self.state.ready() {
            return;
        }
        let Some(folder) = self.state.folder() else {
            return;
        };
        let request = Request {
            out: folder.join("report.html"),
            folder,
            write_digest: self.state.write_digest,
            embed_fonts: self.state.embed_fonts,
        };
        self.state.running = true;
        self.state.report = None;
        self.state.stage = Some(0);
        self.state.status = "Reading the export".into();
        self.progress = Some(job::spawn(request, ctx.clone()));
    }

    /// Apply everything the worker has said since the last frame.
    ///
    /// In a batch: `tga_read::load` emits one message per `result.json`, and a
    /// ten-topic export would otherwise be ten frames nobody sees.
    fn drain(&mut self) {
        let Some(rx) = &self.progress else {
            return;
        };
        let batch: Vec<job::Progress> = rx.try_iter().collect();
        for event in batch {
            self.state.apply(event);
        }
        if !self.state.running {
            // The worker is finished with; dropping the receiver is what lets a
            // second run start clean.
            self.progress = None;
        }
    }

    fn browse(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Choose an export folder")
            .pick_folder()
        {
            self.state.set_path(&dir);
        }
    }

    /// A folder — or a database — dropped anywhere on the window.
    ///
    /// The window is the target rather than the field, which is a small one.
    /// The first directory wins, or the first file that is a database: those
    /// are the two things an export can be. Dropping `result.json` itself is
    /// still a reasonable mistake and still does nothing rather than
    /// half-working.
    fn dropped(&mut self, ctx: &egui::Context) {
        let dirs: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| p.is_dir() || tga_db::find(p).is_some())
                .collect()
        });
        if let Some(dir) = dirs.into_iter().next() {
            self.state.set_path(&dir);
        }
    }

    fn open_report(&mut self) {
        let Some(path) = self.state.report.clone() else {
            return;
        };
        if let Err(e) = open::open(&path) {
            // The report is still on disk and still valid; only the handing-off
            // failed. The button stays live so the user can try again.
            self.state.status = e;
        }
    }
}

impl eframe::App for Shell {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Palette::BG.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();
        self.dropped(ctx);

        let running = self.state.running;
        let can_run = self.state.ready();
        let can_open = self.state.report.is_some() && !running;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Palette::BG)
                    .inner_margin(egui::Margin {
                        left: 34,
                        right: 34,
                        top: 30,
                        bottom: 26,
                    }),
            )
            .show(ctx, |ui| {
                ui.label(caps(TITLE, Palette::FG));
                ui.add_space(8.0);
                ui.label(RichText::new(BLURB).size(size::BODY).color(Palette::MUTED));
                ui.add_space(22.0);
                rule(ui);
                ui.add_space(20.0);

                // -- the folder ------------------------------------------
                ui.label(caps("Export folder", Palette::MUTED));
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    let button = ui.available_width() - 110.0;
                    // The field is a plain `TextEdit`: a caret, a selection and
                    // a clipboard, which is the one control worth borrowing
                    // rather than drawing. Pasting a path out of Explorer is how
                    // this window is actually used.
                    let field = ui.add_sized(
                        [button.max(120.0), 30.0],
                        egui::TextEdit::singleline(&mut self.state.path)
                            .font(theme::mono(size::SMALL))
                            .text_color(Palette::FG)
                            .hint_text(
                                RichText::new(r"C:\...\UA KOLAB")
                                    .font(theme::mono(size::SMALL))
                                    .color(Palette::MUTED),
                            )
                            .margin(egui::Margin::symmetric(8, 6)),
                    );
                    if field.changed() {
                        self.state.revalidate();
                    }
                    if ui.add(flat("Choose", true, false)).clicked() {
                        self.browse();
                    }
                });
                ui.add_space(20.0);

                // -- the options -----------------------------------------
                // No theme control. The design is dark, the report it writes is
                // dark, and a switch between one thing is a control that only
                // ever reports its own existence.
                ui.label(caps("Also", Palette::MUTED));
                ui.add_space(7.0);
                ui.add_enabled_ui(!running, |ui| {
                    ui.checkbox(
                        &mut self.state.write_digest,
                        RichText::new("Write the digest an AI needs to map events")
                            .size(size::BODY),
                    );
                    ui.checkbox(
                        &mut self.state.embed_fonts,
                        RichText::new("Embed the fonts (keeps the report standalone)")
                            .size(size::BODY),
                    );
                });

                // -- the footer ------------------------------------------
                let footer = 70.0;
                let room = ui.available_height() - footer;
                if room > 0.0 {
                    ui.add_space(room);
                }
                rule(ui);
                ui.add_space(14.0);
                bar(ui, self.state.fill());
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(flat("Analyse", can_run, true)).clicked() {
                            self.start(ctx);
                        }
                        if ui.add(flat("Open report", can_open, false)).clicked() {
                            self.open_report();
                        }
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(&self.state.status)
                                .font(theme::mono(size::SMALL))
                                .color(Palette::MUTED),
                        );
                    });
                });
            });
    }
}

/// Letterspaced uppercase micro-type, as the report's section headings are.
///
/// The spacing is put in by hand because egui has no tracking property, and
/// without it a 10px uppercase line reads as a clot rather than as a label.
fn caps(text: &str, colour: Color32) -> RichText {
    let spaced: String = text
        .to_uppercase()
        .chars()
        .flat_map(|c| [c, '\u{2009}'])
        .collect();
    RichText::new(spaced)
        .font(theme::mono(size::MICRO))
        .color(colour)
}

/// A hairline across the measure. One pixel, and the divider is the design.
fn rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0_f32, Palette::RULE),
    );
}

/// A square button, and a disabled one that says so by going quiet.
///
/// Disabled is drawn rather than hidden: a control that vanishes when it cannot
/// be used takes the explanation of *why* with it.
///
/// `primary` fills with the one red, and exactly one control on the page gets
/// it — an accent that marks two things marks neither.
fn flat(label: &str, enabled: bool, primary: bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| {
        let (fill, text, edge) = match (enabled, primary) {
            (true, true) => (Palette::ACCENT, Palette::ACCENT_FG, Palette::ACCENT),
            (true, false) => (Palette::BG, Palette::FG, Palette::HAIRLINE),
            (false, _) => (Palette::BG, Palette::MUTED, Palette::RULE),
        };
        let button = egui::Button::new(RichText::new(label).size(size::BODY).color(text))
            .corner_radius(0.0)
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, edge));
        ui.add_enabled(enabled, button)
    }
}

/// The progress bar: 3px, flat, and determinate always.
fn bar(ui: &mut egui::Ui, fraction: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 3.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Palette::RULE);
    // NaN would poison the layout rather than draw nothing, so it is clamped
    // into range on the way in.
    let done = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if done > 0.0 {
        let mut filled = rect;
        filled.set_width(rect.width() * done);
        painter.rect_filled(filled, 0.0, Palette::ACCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caps_label_keeps_every_letter_it_was_given() {
        // The tracking is built out of thin spaces, so a bug here silently drops
        // or duplicates characters in every heading in the window.
        let text = caps("Export folder", Palette::FG);
        let plain: String = text.text().chars().filter(|c| *c != '\u{2009}').collect();
        assert_eq!(plain, "EXPORT FOLDER");
    }
}
