//! SVG marks, drawn from Rust, themed from CSS.
//!
//! Ported from `analyser/charts.py`.
//!
//! Two rules run through all of it.
//!
//! **Colour is a variable, never a hex.** Every fill is `var(--accent)` or a
//! ramp step `var(--r3)`, so the report's light/dark switch is a class on
//! `<html>` and not a re-render. Nothing here needs to know which theme is on.
//!
//! **Every mark grows from one baseline and nothing gets a second axis.** Two
//! measures on one plot with two scales invent a correlation the data does not
//! contain; where the report has two measures it draws two charts on the same
//! x-axis instead, which is the same comparison without the lie.
//!
//! Sizes follow the house specs: bars capped so the band keeps its air, a 2px
//! surface gap between neighbours, hairline solid gridlines one step off the
//! surface, and the value on the extreme rather than on every mark.

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::{Datelike, Days, NaiveDate};
use tga_notes::Event;

/// A bar narrower than this cannot be hovered or seen, so a series with more
/// points than the width allows is bucketed until each bar clears it.
pub const MIN_BAR: f64 = 3.0;
/// Never let a bar fill its whole band — the gap is what separates it from its
/// neighbour, and it is the surface showing through, not a stroke.
pub const BAR_GAP: f64 = 2.0;
pub const MAX_BAR: f64 = 24.0;

/// One day of the shared time axis.
pub type Day = (String, i64);

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

/// Everything interpolated into markup goes through this, no exceptions.
///
/// A report opens as a local file, so anything that survives as markup runs
/// with that origin — and every name, filename, emoji and link in here came
/// from strangers on the internet.
pub fn esc(text: &str) -> String {
    // `&` first, or the ampersands this function introduces get escaped again.
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Trim `text` to what fits in `room` px, with an ellipsis.
///
/// SVG has no text wrapping and this report sets `overflow:visible` so marks
/// can carry labels, which together mean an over-long name is drawn straight
/// off the left edge of the page instead of being clipped. The full string
/// stays in the tooltip. `size` is the measured average advance of Geist Mono
/// at 11px.
pub fn clip(text: &str, room: f64, size: f64) -> String {
    let limit = ((room / size) as usize).max(4);
    // Characters, not bytes: a name in Cyrillic is half as many characters as
    // it is bytes, and slicing on bytes would cut one in half.
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit - 1).collect();
    format!("{head}…")
}

/// The default advance, for callers that do not measure their own.
pub fn clip_default(text: &str, room: f64) -> String {
    clip(text, room, 6.05)
}

/// A coordinate, trimmed. Three decimals is well under a pixel.
pub fn num(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0');
    let text = text.trim_end_matches('.');
    if text.is_empty() {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// A count, grouped for reading.
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

/// `thousands` of a float, truncated toward zero as Python's `int()` is.
pub fn thousands_f(value: f64) -> String {
    thousands(value.trunc() as i64)
}

// ---------------------------------------------------------------------------
// the box
// ---------------------------------------------------------------------------

/// A responsive SVG box. Width is a viewBox unit, not a pixel.
///
/// Scaling is uniform (`meet`), never stretched: `none` would fit the box
/// exactly and squash every circle into an ellipse and every label into
/// condensed type on the way. The page's measure is close to the viewBox width,
/// so the type still lands near its intended size.
pub fn frame(width: f64, height: f64, body: &str, cls: &str, label: &str) -> String {
    format!(
        "<svg class=\"chart {cls}\" viewBox=\"0 0 {} {}\" \
         preserveAspectRatio=\"xMinYMin meet\" role=\"img\" aria-label=\"{}\">{body}</svg>",
        num(width),
        num(height),
        esc(label)
    )
}

/// Which ramp step a value lands on, linearly. Zero is the track.
///
/// Correct for a bar's colour, wrong for a heatmap — see [`Quantiles`].
pub fn ramp_var(value: f64, top: f64, steps: usize) -> String {
    if value <= 0.0 || top <= 0.0 {
        return "var(--track)".to_string();
    }
    let index = ((value / top * steps as f64) as usize + 1).min(steps);
    format!("var(--r{index})")
}

// ---------------------------------------------------------------------------
// the scale
// ---------------------------------------------------------------------------

/// Rank-based bins for a heatmap, plus the cut points to print.
///
/// Chat activity is long-tailed: in the reference archive one day carries 557
/// messages and the median active day carries 19, so binning linearly on the
/// maximum puts **116 of 133 active days in the first shade** and the other
/// four go unused. The grid comes out one flat colour and says nothing.
///
/// Splitting on quantiles instead spends all five shades. The cost is that a
/// shade then means *rank* rather than *magnitude* — twice the colour is not
/// twice the messages — so the report prints the cut points beside the key and
/// every cell keeps its real number in the tooltip and in the table view.
/// Stating the edges is what makes this honest rather than merely prettier.
///
/// Zeroes are excluded from the split: a day nobody spoke is not the quiet end
/// of the scale, it is off the scale, and it gets the track colour.
#[derive(Debug, Clone)]
pub struct Quantiles {
    pub steps: usize,
    pub edges: Vec<i64>,
    pub top: i64,
}

impl Quantiles {
    pub fn new(values: impl IntoIterator<Item = i64>) -> Self {
        Self::with_steps(values, 5)
    }

    pub fn with_steps(values: impl IntoIterator<Item = i64>, steps: usize) -> Self {
        let mut ordered: Vec<i64> = values.into_iter().filter(|v| *v > 0).collect();
        ordered.sort_unstable();

        let top = ordered.last().copied().unwrap_or(0);
        let mut this = Quantiles {
            steps,
            edges: Vec::new(),
            top,
        };
        if ordered.is_empty() {
            return this;
        }

        let cuts = |source: &[i64]| -> Vec<i64> {
            let mut picked: Vec<i64> = (1..steps)
                .map(|i| source[(source.len() * i / steps).min(source.len() - 1)])
                .collect();
            picked.sort_unstable();
            picked.dedup();
            picked
        };

        let mut distinct = ordered.clone();
        distinct.dedup();

        if distinct.len() <= steps {
            // Few enough values that each can have its own shade. Splitting on
            // quantiles here would put four of the five cut points on the same
            // repeated value and waste the ramp again.
            this.edges = distinct[1..].to_vec();
            return this;
        }
        this.edges = cuts(&ordered);
        if this.edges.len() < steps - 1 {
            // Quantiles of the values collapsed onto a repeated low value — a
            // chat where most active days carry one or two messages does this.
            // Split the distinct values instead, which spends the ramp on the
            // range rather than on the mode.
            this.edges = cuts(&distinct);
        }
        this
    }

    /// Which shade a value lands on: `0` is the track, `1..=steps` the ramp.
    ///
    /// [`Self::var`] formats this; [`strip_compact`] groups by it, which is
    /// what lets a whole row of one shade become a single path.
    pub fn index(&self, value: i64) -> usize {
        if value <= 0 {
            return 0;
        }
        let mut step = 1;
        for edge in &self.edges {
            if value >= *edge {
                step += 1;
            }
        }
        step.min(self.steps)
    }

    pub fn var(&self, value: i64) -> String {
        match self.index(value) {
            0 => "var(--track)".to_string(),
            step => format!("var(--r{step})"),
        }
    }

    /// The cut points, as prose, so the key can be read as numbers.
    pub fn caption(&self, unit: &str) -> String {
        if self.edges.is_empty() {
            return String::new();
        }
        let cuts: Vec<String> = self.edges.iter().map(|e| thousands(*e)).collect();
        let tail = if unit.is_empty() {
            String::new()
        } else {
            format!(" {unit}")
        };
        format!(
            "Five shades, one per fifth of the range actually used; the cut points are {} and {}{tail}.",
            cuts.join(", "),
            thousands(self.top)
        )
    }
}

// ---------------------------------------------------------------------------
// the spine
// ---------------------------------------------------------------------------

/// One bar of the shared axis: first day, last day, total.
pub type Bucket = (String, String, i64);

/// Group a daily series until each bar is at least [`MIN_BAR`] wide.
///
/// Returns `(buckets, days_per_bucket)`. A five-year archive is 1,800 days and
/// no screen has 1,800 hoverable columns; bucketing is honest about that, and
/// the report says how wide a bar is.
pub fn bucket_days(series: &[Day], width: f64) -> (Vec<Bucket>, usize) {
    if series.is_empty() {
        return (Vec::new(), 1);
    }
    let per = if width > 0.0 {
        ((series.len() as f64 * MIN_BAR / width).ceil() as usize).max(1)
    } else {
        1
    };
    if per == 1 {
        return (
            series
                .iter()
                .map(|(day, count)| (day.clone(), day.clone(), *count))
                .collect(),
            1,
        );
    }
    let mut out = Vec::with_capacity(series.len() / per + 1);
    for chunk in series.chunks(per) {
        out.push((
            chunk[0].0.clone(),
            chunk[chunk.len() - 1].0.clone(),
            chunk.iter().map(|(_, c)| c).sum(),
        ));
    }
    (out, per)
}

fn bucket_label(first: &str, last: &str) -> String {
    if first == last {
        first.to_string()
    } else {
        format!("{first} to {last}")
    }
}

/// Every bucket's label for a series drawn at `width`, for the shared axis.
///
/// [`strip_compact`] leaves these out of its cells; they are written into the
/// document once and every row's tooltip is composed from them. Emitting them
/// per cell is what made the presence rows 64% of the file.
pub fn bucket_labels(series: &[Day], width: f64) -> Vec<String> {
    bucket_days(series, width)
        .0
        .iter()
        .map(|(first, last, _)| bucket_label(first, last))
        .collect()
}

/// Messages per day as columns on a shared time axis.
///
/// `top` is the value that reaches full height. Passing the whole archive's
/// maximum into every ribbon is what makes a stack of them comparable —
/// rescaled per row, a person who sent four messages and a person who sent four
/// hundred both draw a full-height bar.
pub fn ribbon(
    series: &[Day],
    width: f64,
    height: f64,
    top: Option<i64>,
    tips: bool,
    cls: &str,
) -> String {
    let (buckets, per) = bucket_days(series, width);
    if buckets.is_empty() {
        return frame(width, height, "", "", "");
    }
    let ceiling = top
        .unwrap_or_else(|| buckets.iter().map(|(_, _, c)| *c).max().unwrap_or(1))
        .max(1) as f64;
    let slot = width / buckets.len() as f64;
    let gap = if slot > MIN_BAR + BAR_GAP {
        BAR_GAP
    } else {
        0.0
    };
    let bar = (slot - gap).clamp(1.0, MAX_BAR);

    let mut parts = format!(
        "<rect class=\"ribbon-bed\" x=\"0\" y=\"{}\" width=\"{}\" height=\"1\"/>",
        num(height - 1.0),
        num(width)
    );
    for (index, (first, last, count)) in buckets.iter().enumerate() {
        if *count <= 0 {
            continue;
        }
        let tall = (height * (*count as f64 / ceiling).min(1.0)).max(1.5);
        let x = index as f64 * slot + (slot - bar) / 2.0;
        let radius = (bar / 2.0).min(2.0);
        let tip = if tips {
            format!(
                " data-tip=\"{} &#183; {}\"",
                esc(&bucket_label(first, last)),
                thousands(*count)
            )
        } else {
            String::new()
        };
        let _ = write!(
            parts,
            "<rect class=\"ribbon-bar\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{tip}/>",
            num(x),
            num(height - tall),
            num(bar),
            num(tall),
            num(radius)
        );
    }
    let cls = format!("ribbon {cls}");
    let label = if per == 1 {
        "Messages per day".to_string()
    } else {
        format!("Messages per {per} days")
    };
    frame(width, height, &parts, cls.trim(), &label)
}

/// One person's activity as a heat row on the shared time axis.
///
/// A row of columns scaled to a shared ceiling is the right mark for the
/// archive as a whole and the wrong one for a table of forty people: with one
/// person on 897 messages and the thirtieth on 35, every row past the top few
/// draws as a scatter of 1px dots and reads as a rendering fault.
///
/// Shading the row instead keeps both halves of the question answerable. *Was
/// this person here* is presence, which is visible at any volume, and *how loud
/// were they* is the shade, on a scale shared by every row — so the rows still
/// compare, which was the point of not rescaling them.
///
/// Bucketed with [`bucket_days`] exactly as the hero ribbon is, so a column here
/// sits under the same days as the column above it.
///
/// It is drawn as one path per shade rather than one rect per cell.
///
/// **This is the whole size budget.** Drawn the obvious way — a `<rect>` per
/// cell, each carrying its own `fill` and its own `data-tip` — the KRGM
/// archive's 260 rows (250 people and 10 topics) came to 969 KB of a 1,519 KB
/// report: 64% of the file, against 275 KB for the embedded type and 275 KB for
/// everything else on the page. Nothing else in the document was worth
/// optimising until this was.
///
/// Three things go, and none of them is a fact:
///
/// * **The per-cell `fill`** becomes a class. `.strip .c3{fill:var(--r3)}` is
///   stated once for the document instead of 8,000 times in it.
/// * **The per-cell `<rect>`** becomes a subpath. Cells sharing a shade share
///   one element, so a row is at most five elements however many buckets it
///   has, and a run of equal-shaded days costs 21 characters instead of 128.
/// * **The per-cell `data-tip`** becomes one `data-n` on the row. Every bucket
///   is the same width, so the pointer's x *is* the bucket index; the tooltip
///   is composed from that and the shared axis labels rather than repeated into
///   every cell. It also improves the thing it shrinks — hover now answers
///   anywhere along the row, including over the silent days, where before there
///   was no element to hover at all.
///
/// **The chart is still entirely server-side**, which is the rule that matters:
/// with scripting off the row draws exactly as it does with scripting on, and
/// only the tooltip is missing — and the tooltip was never anything but JS.
///
/// `data-n` is the non-zero buckets as `index:count` pairs. Dense would be one
/// entry per bucket per row, and these rows are mostly silence.
pub fn strip_compact(
    series: &[Day],
    width: f64,
    height: f64,
    scale: &Quantiles,
    cls: &str,
) -> String {
    let (buckets, _) = bucket_days(series, width);
    if buckets.is_empty() {
        return frame(width, height, "", "", "");
    }
    let slot = width / buckets.len() as f64;
    let bar = (slot - 0.5).max(1.0);

    // One path per ramp step. Index 0 is the track and is not drawn: a day
    // nobody spoke is the absence of a mark, not a pale one.
    let mut shades: Vec<String> = vec![String::new(); scale.steps + 1];
    let mut counts: Vec<String> = Vec::new();
    for (index, (_, _, count)) in buckets.iter().enumerate() {
        if *count <= 0 {
            continue;
        }
        let shade = scale.index(*count);
        // `M<x> 0h<w>v<h>h-<w>z` — the rect, as a subpath. Absolute move, then
        // relative, because relative is shorter and the move already fixed the
        // origin.
        let _ = write!(
            shades[shade],
            "M{} 0h{}v{}h-{}z",
            num(index as f64 * slot),
            num(bar),
            num(height),
            num(bar)
        );
        counts.push(format!("{index}:{count}"));
    }

    let mut parts = String::new();
    for (shade, data) in shades.iter().enumerate() {
        if shade == 0 || data.is_empty() {
            continue;
        }
        let _ = write!(parts, "<path class=\"c{shade}\" d=\"{data}\"/>");
    }

    let cls = format!("strip {cls}");
    format!(
        "<svg class=\"chart {}\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"xMinYMin meet\" \
         role=\"img\" aria-label=\"Activity over time\" data-n=\"{}\" data-b=\"{}\">{parts}</svg>",
        cls.trim(),
        num(width),
        num(height),
        counts.join(","),
        buckets.len()
    )
}

fn iso(day: &str) -> NaiveDate {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap_or_default()
}

/// The three-letter month, in English, as `strftime("%b")` gives it.
fn month_abbrev(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES[(month as usize - 1).min(11)]
}

/// Month or year ticks under the spine, thinned to what fits.
pub fn time_axis(series: &[Day], width: f64, height: f64) -> String {
    if series.is_empty() {
        return frame(width, height, "", "", "");
    }
    let first = iso(&series[0].0);
    let last = iso(&series[series.len() - 1].0);
    let span = ((last - first).num_days()).max(1);

    let mut marks: Vec<(NaiveDate, String)> = Vec::new();
    if span > 900 {
        for year in first.year()..=last.year() {
            if let Some(day) = NaiveDate::from_ymd_opt(year, 1, 1) {
                marks.push((day, year.to_string()));
            }
        }
    } else {
        let every = if span <= 200 {
            1
        } else if span <= 500 {
            2
        } else {
            3
        };
        let (mut year, mut month) = (first.year(), first.month());
        let mut step = 0usize;
        while let Some(mark) = NaiveDate::from_ymd_opt(year, month, 1) {
            if mark > last {
                break;
            }
            if step.is_multiple_of(every) {
                let mut label = month_abbrev(mark.month()).to_string();
                // The year is worth stating at January and at the very first
                // mark, and nowhere else — repeating it on every tick is the
                // repeated chrome the design exists to remove.
                if mark.month() == 1 || marks.is_empty() {
                    let _ = write!(label, " {:02}", mark.year().rem_euclid(100));
                }
                marks.push((mark, label));
            }
            step += 1;
            month += 1;
            if month > 12 {
                year += 1;
                month = 1;
            }
        }
    }

    let mut parts = String::new();
    for (when, label) in marks {
        if when < first {
            continue;
        }
        let x = (when - first).num_days() as f64 / span as f64 * width;
        let _ = write!(
            parts,
            "<line class=\"tick\" x1=\"{}\" y1=\"0\" x2=\"{}\" y2=\"4\"/>\
             <text class=\"axis\" x=\"{}\" y=\"13\">{}</text>",
            num(x),
            num(x),
            num((x + 4.0).min(width - 1.0)),
            esc(&label)
        );
    }
    frame(width, height, &parts, "axis-strip", "")
}

/// Event markers pinned to the same axis as the ribbon below them.
///
/// A span draws as a bar with a marker at its start; a moment draws as a marker
/// alone. The marker is 9px so it can be hit, and it carries the surface ring
/// that keeps it legible where two events nearly collide.
pub fn event_rail(events: &[Event], series: &[Day], width: f64, height: f64) -> String {
    rail(events, series, width, height, &[])
}

/// The four marker shapes, in the order kinds are first seen.
///
/// **Shape, not hue, and that is not a preference.** `_timeline` colour-codes
/// its four note kinds green/blue/pink/orange, and its own README records why
/// that failed: normal-vision ΔE 19.3 against CVD ΔE 6.9, which forced
/// shape-coding regardless. Once the shapes carry the distinction the colour is
/// redundant — and this design has one hue to spend, which magnitude has a
/// better claim on. Past four kinds the shapes repeat, and the filter row and
/// the card both name the kind in words, so nothing rests on the shape alone.
const SHAPES: usize = 4;

/// The marker for one kind, drawn at the origin so the group can place it.
fn marker(shape: usize, r: f64) -> String {
    match shape % SHAPES {
        // ● a circle
        0 => format!("<circle class=\"ev-dot\" r=\"{}\"/>", num(r)),
        // ■ a square, sized to the circle's area rather than its diameter, or
        // it reads as the larger mark
        1 => {
            let s = r * 1.77;
            format!(
                "<rect class=\"ev-dot\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                num(-s / 2.0),
                num(-s / 2.0),
                num(s),
                num(s)
            )
        }
        // ▲ a triangle, likewise
        2 => {
            let s = r * 1.35;
            format!(
                "<polygon class=\"ev-dot\" points=\"0,{} {},{} {},{}\"/>",
                num(-s * 1.15),
                num(s),
                num(s * 0.66),
                num(-s),
                num(s * 0.66)
            )
        }
        // ◆ a diamond
        _ => {
            let s = r * 1.25;
            format!(
                "<polygon class=\"ev-dot\" points=\"0,{} {},0 0,{} {},0\"/>",
                num(-s),
                num(s),
                num(s),
                num(-s)
            )
        }
    }
}

/// Event markers pinned to the same axis as the ribbon below them.
///
/// `kinds` is the vocabulary the notes file actually used, in first-seen order.
/// Non-empty shape-codes each kind and adds the `k<n>` class the filter row
/// toggles; empty renders plain circles, which is what a notes file whose
/// entries carry no `kind` at all gets.
pub fn rail(events: &[Event], series: &[Day], width: f64, height: f64, kinds: &[String]) -> String {
    if series.is_empty() || events.is_empty() {
        return String::new();
    }
    let first = iso(&series[0].0);
    let last = iso(&series[series.len() - 1].0);
    let span = ((last - first).num_days()).max(1) as f64;

    // Inset, so a marker on the first or last day keeps its whole dot and its
    // ring inside the box instead of being sliced by the edge.
    let inset = 6.0;
    let inner = (width - inset * 2.0).max(1.0);

    let mut parts = String::new();
    for (index, event) in events.iter().enumerate() {
        let at = |day: NaiveDate| -> f64 {
            inset + ((day - first).num_days() as f64 / span).clamp(0.0, 1.0) * inner
        };
        let x = at(event.start);
        let row = index % 3;
        let y = 8.0 + row as f64 * 8.0;
        if event.spans() {
            if let Some(end) = event.end {
                let end_x = at(end);
                let _ = write!(
                    parts,
                    "<rect class=\"ev-span\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"3\" rx=\"1.5\"/>",
                    num(x),
                    num(y - 1.5),
                    num((end_x - x).max(2.0))
                );
            }
        }
        let conf = if event.confidence.is_empty() {
            String::new()
        } else {
            format!(" conf-{}", esc(&event.confidence))
        };

        if kinds.is_empty() {
            // A plain dot: no kind was named, so there is nothing to shape-code
            // and no filter row for a class to talk to.
            let _ = write!(
                parts,
                "<g class=\"ev{conf}\" data-event=\"{}\" tabindex=\"0\" role=\"button\" aria-label=\"{}\">\
                 <line class=\"ev-stem\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>\
                 <circle class=\"ev-hit\" cx=\"{}\" cy=\"{}\" r=\"9\"/>\
                 <circle class=\"ev-dot\" cx=\"{}\" cy=\"{}\" r=\"4\"/></g>",
                esc(&event.id),
                esc(&event.title),
                num(x),
                num(y),
                num(x),
                num(height),
                num(x),
                num(y),
                num(x),
                num(y)
            );
            continue;
        }

        let shape = kinds.iter().position(|k| *k == event.kind).unwrap_or(0);
        // Weight scales the marker. `major` is the default size rather than an
        // enlargement, so a file that states no weights looks like one that
        // states them all as major — which is the honest reading of silence.
        let r = match event.weight.as_str() {
            "minor" => 2.6,
            "medium" => 3.3,
            _ => 4.0,
        };
        let _ = write!(
            parts,
            "<g class=\"ev k{shape}{conf}\" data-event=\"{}\" data-kind=\"{}\" tabindex=\"0\" \
             role=\"button\" aria-label=\"{}\">\
             <line class=\"ev-stem\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>\
             <circle class=\"ev-hit\" cx=\"{}\" cy=\"{}\" r=\"9\"/>\
             <g transform=\"translate({},{})\">{}</g></g>",
            esc(&event.id),
            esc(&event.kind),
            esc(&event.title),
            num(x),
            num(y),
            num(x),
            num(height),
            num(x),
            num(y),
            num(x),
            num(y),
            marker(shape, r)
        );
    }
    frame(width, height, &parts, "rail", "Events")
}

/// How much of the archive the notes claim to have read, on the shared axis.
///
/// **Coverage is the field that changes what the page means.** A timeline with
/// markers over one month and nothing over
/// the next nine looks identical whether the rest was quiet or simply unread,
/// and those are opposite facts about the same picture. The band draws the read
/// span against the whole span, so the unread part is visibly unread rather
/// than apparently uneventful.
///
/// An open end — `from` with no `to`, or neither — is drawn as far as it is
/// claimed and no further. Guessing the missing end would be inventing the
/// claim the block exists to state.
pub fn coverage_band(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    series: &[Day],
    width: f64,
    height: f64,
) -> String {
    if series.is_empty() || (from.is_none() && to.is_none()) {
        return String::new();
    }
    let first = iso(&series[0].0);
    let last = iso(&series[series.len() - 1].0);
    let span = ((last - first).num_days()).max(1) as f64;
    let at = |day: NaiveDate| -> f64 {
        ((day - first).num_days() as f64 / span).clamp(0.0, 1.0) * width
    };

    let start = from.map(at).unwrap_or(0.0);
    let end = to.map(at).unwrap_or(width);
    let body = format!(
        "<rect class=\"cov-bed\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/>\
         <rect class=\"cov-read\" x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\"/>",
        num(width),
        num(height),
        num(start.min(end)),
        num((end - start).abs().max(1.0)),
        num(height)
    );
    frame(
        width,
        height,
        &body,
        "cov",
        "How much of the archive was read",
    )
}

// ---------------------------------------------------------------------------
// columns, bars, histograms
// ---------------------------------------------------------------------------

/// A column per slot, one hue, the extreme labelled and nothing else.
pub fn columns(
    labels: &[String],
    values: &[i64],
    width: f64,
    height: f64,
    label_every: usize,
    unit: &str,
    highlight: Option<usize>,
) -> String {
    if values.is_empty() {
        return frame(width, height, "", "", "");
    }
    // `max(values) or 1` used to stand in for an empty series, which turned a
    // genuine all-zero series into a top of 1 that no value equals — and the
    // peak lookup below then failed. An export with no participants.json has
    // exactly that shape: every month's arrivals is 0. A series with nothing in
    // it draws its baseline and its axis and no bars, which is the true
    // picture, rather than failing the whole report.
    let peak_value = *values.iter().max().expect("non-empty");
    let top = if peak_value == 0 { 1 } else { peak_value } as f64;
    // Headroom for the one value that gets a direct label. Without it the
    // tallest bar reaches the top of the box and its label is drawn outside,
    // where it survives only because the SVG does not clip.
    let headroom = 15.0;
    let plot = height - 18.0;
    let slot = width / values.len() as f64;
    let bar = (slot - BAR_GAP).clamp(1.0, MAX_BAR);
    let peak: Option<usize> = if peak_value != 0 {
        values.iter().position(|v| *v == peak_value)
    } else {
        None
    };
    let label_every = label_every.max(1);

    let mut parts = format!(
        "<line class=\"baseline\" x1=\"0\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        num(plot),
        num(width),
        num(plot)
    );
    let axis_label = |index: usize| -> String {
        format!(
            "<text class=\"axis\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(index as f64 * slot + slot / 2.0),
            num(height - 5.0),
            esc(&labels[index])
        )
    };
    for (index, value) in values.iter().enumerate() {
        let x = index as f64 * slot + (slot - bar) / 2.0;
        let tall = (plot - headroom) * (*value as f64 / top);
        let cls = if Some(index) == peak || Some(index) == highlight {
            "col peak"
        } else {
            "col"
        };
        if *value <= 0 {
            // A zero-height rect is not a mark. It cannot be seen or hovered,
            // and leaving it in claims the chart drew something it did not.
            if index % label_every == 0 {
                parts.push_str(&axis_label(index));
            }
            continue;
        }
        let _ = write!(
            parts,
            "<rect class=\"{cls}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" \
             data-tip=\"{} &#183; {}{}\"/>",
            num(x),
            num(plot - tall),
            num(bar),
            num(tall.max(0.0)),
            num((bar / 2.0).min(4.0)),
            esc(&labels[index]),
            thousands(*value),
            esc(unit)
        );
        if index % label_every == 0 {
            parts.push_str(&axis_label(index));
        }
    }
    if let Some(peak) = peak {
        let _ = write!(
            parts,
            "<text class=\"value\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(peak as f64 * slot + slot / 2.0),
            num(plot - (plot - headroom) - 5.0),
            thousands(peak_value)
        );
    }
    frame(width, height, &parts, "columns", "")
}

/// One ranked row: label, value, and the text to print at the tip.
pub type BarRow = (String, f64, String);

/// Ranked horizontal bars: label, bar, value at the tip.
///
/// Horizontal because the labels are people's names and topic titles, which do
/// not fit under a column and must never be rotated to make them.
pub fn bars_h(
    rows: &[BarRow],
    width: f64,
    row_height: f64,
    label_width: f64,
    value_width: f64,
) -> String {
    if rows.is_empty() {
        return frame(width, row_height, "", "", "");
    }
    let top = {
        let biggest = rows.iter().map(|(_, v, _)| *v).fold(f64::MIN, f64::max);
        if biggest == 0.0 {
            1.0
        } else {
            biggest
        }
    };
    let plot = width - label_width - value_width;
    let height = row_height * rows.len() as f64;
    let bar = (row_height - 8.0).min(MAX_BAR);

    let mut parts = String::new();
    for (index, (label, value, note)) in rows.iter().enumerate() {
        let y = index as f64 * row_height;
        let length = plot * (*value / top);
        let shown = if note.is_empty() {
            thousands_f(*value)
        } else {
            note.clone()
        };
        let _ = write!(
            parts,
            "<text class=\"rowlabel\" x=\"{}\" y=\"{}\" text-anchor=\"end\" \
             data-tip=\"{} &#183; {}\">{}</text>",
            num(label_width - 10.0),
            num(y + row_height / 2.0 + 4.0),
            esc(label),
            esc(&shown),
            esc(&clip_default(label, label_width - 12.0))
        );
        let _ = write!(
            parts,
            "<rect class=\"bar\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" \
             data-tip=\"{} &#183; {}\"/>",
            num(label_width),
            num(y + (row_height - bar) / 2.0),
            num(length.max(1.0)),
            num(bar),
            num((bar / 2.0).min(4.0)),
            esc(label),
            esc(&shown)
        );
        let _ = write!(
            parts,
            "<text class=\"value\" x=\"{}\" y=\"{}\">{}</text>",
            num(label_width + length + 8.0),
            num(y + row_height / 2.0 + 4.0),
            esc(&shown)
        );
    }
    frame(width, height, &parts, "bars", "")
}

// ---------------------------------------------------------------------------
// grids
// ---------------------------------------------------------------------------

/// Weekday against hour. Magnitude, so a single-hue ramp.
pub fn heatgrid(
    row_labels: &[String],
    col_labels: &[String],
    cells: &[Vec<i64>],
    width: f64,
    cell: f64,
    gutter: f64,
    scale: Option<&Quantiles>,
) -> String {
    if cells.is_empty() {
        return frame(width, cell, "", "", "");
    }
    let cols = cells[0].len();
    let size = cell.min((width - gutter) / cols as f64 - BAR_GAP);
    let owned;
    let scale = match scale {
        Some(scale) => scale,
        None => {
            owned = Quantiles::new(cells.iter().flatten().copied());
            &owned
        }
    };
    let height = cells.len() as f64 * (size + BAR_GAP) + 18.0;

    let mut parts = String::new();
    for (r, row) in cells.iter().enumerate() {
        let y = r as f64 * (size + BAR_GAP);
        let _ = write!(
            parts,
            "<text class=\"rowlabel\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
            num(gutter - 10.0),
            num(y + size / 2.0 + 4.0),
            esc(&row_labels[r])
        );
        for (c, value) in row.iter().enumerate() {
            let x = gutter + c as f64 * (size + BAR_GAP);
            let _ = write!(
                parts,
                "<rect class=\"cell\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                 fill=\"{}\" data-tip=\"{} {} &#183; {}\"/>",
                num(x),
                num(y),
                num(size),
                num(size),
                scale.var(*value),
                esc(&row_labels[r]),
                esc(&col_labels[c]),
                thousands(*value)
            );
        }
    }
    for (c, label) in col_labels.iter().enumerate() {
        if c % 3 != 0 {
            continue;
        }
        let x = gutter + c as f64 * (size + BAR_GAP) + size / 2.0;
        let _ = write!(
            parts,
            "<text class=\"axis\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(x),
            num(height - 4.0),
            esc(label)
        );
    }
    frame(width, height, &parts, "heat", "")
}

/// One square per day, weeks as columns, running continuously.
///
/// Not split into a block per year. A year block is the shape everyone knows
/// from a contributions graph, and on an archive that starts in December it
/// produces a three-week stub above a full-width year, which reads as a
/// rendering fault rather than as December. Running the weeks straight through
/// keeps this on the same left-to-right time axis as every other ribbon on the
/// page, and the months are labelled along the top.
///
/// The empty square is the track rather than the ramp's palest step, so a day
/// nobody posted is visibly different from a day one person did.
pub fn calendar(
    series: &[Day],
    width: f64,
    cell: Option<f64>,
    scale: Option<&Quantiles>,
) -> String {
    if series.is_empty() {
        return frame(width, cell.unwrap_or(12.0), "", "", "");
    }
    let counts: HashMap<&str, i64> = series.iter().map(|(d, n)| (d.as_str(), *n)).collect();
    let owned;
    let scale = match scale {
        Some(scale) => scale,
        None => {
            owned = Quantiles::new(series.iter().map(|(_, n)| *n));
            &owned
        }
    };
    let first = iso(&series[0].0);
    let last = iso(&series[series.len() - 1].0);
    let origin = first - Days::new(first.weekday().num_days_from_monday() as u64);
    let weeks = ((last - origin).num_days() / 7 + 1).max(1) as f64;

    let gutter = 34.0;
    let head = 16.0;
    let cell = cell.unwrap_or_else(|| ((width - gutter) / weeks - BAR_GAP).clamp(3.0, 18.0));
    let step = cell + if cell >= 6.0 { BAR_GAP } else { 1.0 };
    let height = head + 7.0 * step + 4.0;

    let mut parts = String::new();
    for (row, name) in ["Mon", "", "Wed", "", "Fri", "", "Sun"].iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let _ = write!(
            parts,
            "<text class=\"axis\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{name}</text>",
            num(gutter - 8.0),
            num(head + row as f64 * step + cell * 0.8)
        );
    }

    let mut day = first;
    while day <= last {
        let week = ((day - origin).num_days() / 7) as f64;
        let x = gutter + week * step;
        let y = head + day.weekday().num_days_from_monday() as f64 * step;
        let iso_day = day.to_string();
        let count = counts.get(iso_day.as_str()).copied().unwrap_or(0);
        let _ = write!(
            parts,
            "<rect class=\"cell\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             fill=\"{}\" data-tip=\"{iso_day} &#183; {}\"/>",
            num(x),
            num(y),
            num(cell),
            num(cell),
            scale.var(count),
            thousands(count)
        );
        day = match day.checked_add_days(Days::new(1)) {
            Some(next) => next,
            None => break,
        };
    }

    // Month labels along the top, thinned until they stop colliding.
    let every = ((46.0 / (step * 4.34).max(1.0)) as usize).max(1);
    let (mut year, mut month) = (first.year(), first.month());
    let mut index = 0usize;
    while let Some(mark) = NaiveDate::from_ymd_opt(year, month, 1) {
        if mark > last {
            break;
        }
        if mark >= origin && index.is_multiple_of(every) {
            let week = ((mark - origin).num_days() / 7) as f64;
            let mut label = month_abbrev(mark.month()).to_string();
            if mark.month() == 1 || index == 0 {
                let _ = write!(label, " {:02}", mark.year().rem_euclid(100));
            }
            let _ = write!(
                parts,
                "<text class=\"axis\" x=\"{}\" y=\"10\">{}</text>",
                num(gutter + week * step),
                esc(&label)
            );
        }
        index += 1;
        month += 1;
        if month > 12 {
            year += 1;
            month = 1;
        }
    }

    frame(width, height, &parts, "calendar", "Messages per day")
}

/// Who replies to whom: rows answer, columns are answered.
pub fn matrix(labels: &[String], cells: &[Vec<i64>], width: f64, gutter: f64) -> String {
    if cells.is_empty() {
        return frame(width, 10.0, "", "", "");
    }
    let size = ((width - gutter) / labels.len().max(1) as f64 - BAR_GAP).clamp(8.0, 22.0);
    // Off-diagonal only: the diagonal is drawn as a neutral and must not be
    // allowed to set the scale for everything else.
    let scale = Quantiles::new(cells.iter().enumerate().flat_map(|(r, row)| {
        row.iter()
            .enumerate()
            .filter_map(move |(c, v)| if r != c { Some(*v) } else { None })
    }));
    let height = labels.len() as f64 * (size + BAR_GAP) + gutter * 0.55;

    let mut parts = String::new();
    for (r, row) in cells.iter().enumerate() {
        let y = r as f64 * (size + BAR_GAP);
        let _ = write!(
            parts,
            "<text class=\"rowlabel\" x=\"{}\" y=\"{}\" text-anchor=\"end\" data-tip=\"{}\">{}</text>",
            num(gutter - 10.0),
            num(y + size / 2.0 + 4.0),
            esc(&labels[r]),
            esc(&clip_default(&labels[r], gutter - 12.0))
        );
        for (c, value) in row.iter().enumerate() {
            let x = gutter + c as f64 * (size + BAR_GAP);
            let fill = if r == c {
                "var(--self)".to_string()
            } else {
                scale.var(*value)
            };
            let _ = write!(
                parts,
                "<rect class=\"cell\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                 fill=\"{fill}\" data-tip=\"{} &#8594; {} &#183; {}\"/>",
                num(x),
                num(y),
                num(size),
                num(size),
                esc(&labels[r]),
                esc(&labels[c]),
                thousands(*value)
            );
        }
    }
    for (c, label) in labels.iter().enumerate() {
        let x = gutter + c as f64 * (size + BAR_GAP) + size / 2.0;
        let y = labels.len() as f64 * (size + BAR_GAP) + 8.0;
        let _ = write!(
            parts,
            "<text class=\"axis colhead\" x=\"{}\" y=\"{}\" transform=\"rotate(45 {} {})\" \
             data-tip=\"{}\">{}</text>",
            num(x),
            num(y),
            num(x),
            num(y),
            esc(label),
            esc(&clip(label, gutter * 0.72, 5.2))
        );
    }
    frame(width, height, &parts, "matrix", "")
}

/// One node of the interaction graph, as `graph::build` emits it.
pub struct Node {
    pub key: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub messages: i64,
    pub degree: i64,
}

/// One pooled, undirected edge.
pub struct Link {
    pub a: String,
    pub b: String,
    pub weight: i64,
}

/// The interaction graph. Undirected, pooled, deterministic.
///
/// Edge weight is opacity and width; node area — not radius — carries the
/// message count, because a reader compares blobs by area and doubling the
/// radius quadruples the ink for twice the data.
pub fn network(
    nodes: &[Node],
    edges: &[Link],
    names: &HashMap<String, String>,
    width: f64,
    height: f64,
) -> String {
    if nodes.is_empty() {
        return frame(width, height, "", "", "");
    }
    let pad = 44.0;
    let (inner_w, inner_h) = (width - pad * 2.0, height - pad * 2.0);
    let place: HashMap<&str, (f64, f64)> = nodes
        .iter()
        .map(|n| (n.key.as_str(), (pad + n.x * inner_w, pad + n.y * inner_h)))
        .collect();
    let heaviest = {
        let top = edges.iter().map(|e| e.weight).max().unwrap_or(1);
        if top == 0 {
            1
        } else {
            top
        }
    } as f64;

    let mut parts = String::new();
    for edge in edges {
        let (Some((x1, y1)), Some((x2, y2))) = (
            place.get(edge.a.as_str()).copied(),
            place.get(edge.b.as_str()).copied(),
        ) else {
            continue;
        };
        let share = edge.weight as f64 / heaviest;
        let _ = write!(
            parts,
            "<line class=\"link\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" \
             stroke-width=\"{}\" opacity=\"{}\"/>",
            num(x1),
            num(y1),
            num(x2),
            num(y2),
            num(0.6 + 2.4 * share),
            num(0.18 + 0.5 * share)
        );
    }
    for node in nodes {
        let (x, y) = place[node.key.as_str()];
        let radius = 4.0 + 13.0 * node.size.max(0.0).sqrt();
        // Truncated hard: at this density a full name is a collision, and the
        // tooltip carries the whole string anyway.
        let full = names
            .get(&node.key)
            .cloned()
            .unwrap_or_else(|| node.key.clone());
        let label = clip(&full, 104.0, 5.4);
        let _ = write!(
            parts,
            "<g class=\"node\" data-tip=\"{} &#183; {} messages, {} exchanges\">\
             <circle class=\"dot\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>\
             <text class=\"nodelabel\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text></g>",
            esc(&label),
            thousands(node.messages),
            thousands(node.degree),
            num(x),
            num(y),
            num(radius),
            num(x),
            num(y - radius - 6.0),
            esc(&label)
        );
    }
    frame(width, height, &parts, "network", "Interaction graph")
}

/// The ramp's key. A single-series chart needs none; a ramp always does.
pub fn legend(steps: usize, low: &str, high: &str) -> String {
    let swatches: String = (0..steps)
        .map(|i| format!("<i style=\"background:var(--r{})\"></i>", i + 1))
        .collect();
    format!(
        "<p class=\"legend\"><span>{}</span><i class=\"swatch-track\"></i>{swatches}<span>{}</span></p>",
        esc(low),
        esc(high)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(counts: &[i64]) -> Vec<Day> {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        counts
            .iter()
            .enumerate()
            .map(|(i, n)| ((start + Days::new(i as u64)).to_string(), *n))
            .collect()
    }

    #[test]
    fn escaping_does_not_double_escape_its_own_ampersands() {
        assert_eq!(
            esc("<a href='x'>&\"</a>"),
            "&lt;a href=&#39;x&#39;&gt;&amp;&quot;&lt;/a&gt;"
        );
    }

    #[test]
    fn a_coordinate_loses_its_trailing_zeros_but_never_becomes_empty() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(1.2345), "1.234"); // half-even, as Python's format is
        assert_eq!(num(12.0625), "12.062");
        // Python: f"{-0.0001:.3f}" -> "-0.000" -> "-0". The point of the test is
        // that neither side produces "" or "-".
        assert_eq!(num(-0.0001), "-0");
    }

    #[test]
    fn a_group_separator_is_a_non_breaking_space() {
        // A plain space here puts "6" at the end of one line and "643" at the
        // start of the next, which is how a count becomes two numbers.
        assert_eq!(thousands(6_643), "6\u{a0}643");
        assert_eq!(thousands(333_582), "333\u{a0}582");
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(-1_234), "-1\u{a0}234");
    }

    #[test]
    fn clipping_counts_characters_not_bytes() {
        // Every character here is two bytes. Clipping on bytes would cut one in
        // half and emit invalid UTF-8 into the middle of an SVG label.
        // 30px at a 6.05px advance is four characters of room, so three
        // survive and the fourth is the ellipsis.
        let name = "ћћћћћћћћћћ";
        assert_eq!(clip(name, 30.0, 6.05), "ћћћ…");
        assert_eq!(clip("short", 300.0, 6.05), "short");
    }

    #[test]
    fn clipping_never_goes_below_four_characters_of_room() {
        assert_eq!(clip("abcdefgh", 0.0, 6.05), "abc…");
    }

    #[test]
    fn a_series_is_bucketed_until_every_bar_can_be_hovered() {
        let (buckets, per) = bucket_days(&series(&[1; 1800]), 1120.0);
        assert_eq!(per, 5, "1800 days at 3px minimum across 1120px");
        assert_eq!(buckets.len(), 360);
        assert_eq!(buckets[0].2, 5, "each bucket sums its days");
        assert_eq!(buckets[0].0, "2025-01-01");
        assert_eq!(buckets[0].1, "2025-01-05");
    }

    #[test]
    fn a_short_series_is_not_bucketed_at_all() {
        let (buckets, per) = bucket_days(&series(&[1, 2, 3]), 1120.0);
        assert_eq!(per, 1);
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].0, buckets[0].1, "one day, so first == last");
    }

    #[test]
    fn an_empty_series_buckets_to_nothing_rather_than_dividing_by_zero() {
        let (buckets, per) = bucket_days(&[], 1120.0);
        assert!(buckets.is_empty());
        assert_eq!(per, 1);
    }

    #[test]
    fn quantiles_spend_the_whole_ramp_on_a_long_tailed_series() {
        // The case the type exists for: one day at 557 and a median of 19.
        let mut values: Vec<i64> = (0..132).map(|i| 1 + (i % 40)).collect();
        values.push(557);
        let scale = Quantiles::new(values);
        let used: std::collections::BTreeSet<String> = (1..=557).map(|v| scale.var(v)).collect();
        assert_eq!(used.len(), 5, "all five shades are reachable");
        assert_eq!(scale.var(0), "var(--track)");
        assert_eq!(scale.var(557), "var(--r5)");
    }

    #[test]
    fn few_distinct_values_get_one_shade_each() {
        // Splitting on quantiles here would put four cut points on the same
        // repeated value and waste the ramp.
        let scale = Quantiles::new([1, 1, 1, 2, 2, 3]);
        assert_eq!(scale.edges, vec![2, 3]);
        assert_eq!(scale.var(1), "var(--r1)");
        assert_eq!(scale.var(2), "var(--r2)");
        assert_eq!(scale.var(3), "var(--r3)");
    }

    #[test]
    fn quantiles_collapsing_onto_the_mode_re_cut_on_the_distinct_values() {
        // A chat where most active days carry one or two messages: the
        // quantiles of the raw values are all 1, so the first cut yields a
        // single edge and the fallback splits the distinct values instead.
        let mut values = vec![1i64; 100];
        values.extend([2, 3, 4, 5, 9, 40]);
        let scale = Quantiles::new(values);
        assert!(
            scale.edges.len() >= 4,
            "the fallback should spend the ramp: {:?}",
            scale.edges
        );
    }

    #[test]
    fn an_all_zero_series_has_no_edges_and_no_caption() {
        let scale = Quantiles::new([0, 0, 0]);
        assert!(scale.edges.is_empty());
        assert_eq!(scale.caption("messages"), "");
        assert_eq!(scale.var(0), "var(--track)");
    }

    #[test]
    fn the_caption_states_the_cut_points_as_numbers() {
        let scale = Quantiles::new([1, 2, 3, 4]);
        let caption = scale.caption("messages");
        assert!(caption.starts_with("Five shades"));
        assert!(caption.ends_with("and 4 messages."));
    }

    #[test]
    fn a_zero_column_draws_its_label_but_not_a_rect() {
        // A zero-height rect cannot be seen or hovered, and leaving it in
        // claims the chart drew something it did not.
        let labels: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let svg = columns(&labels, &[0, 4], 200.0, 100.0, 1, "", None);
        assert_eq!(svg.matches("<rect").count(), 1);
        assert_eq!(svg.matches("class=\"axis\"").count(), 2);
    }

    #[test]
    fn an_all_zero_series_draws_its_axis_and_no_bars_rather_than_failing() {
        // An export with no participants.json has exactly this shape: every
        // month's arrivals is 0.
        let labels: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let svg = columns(&labels, &[0, 0, 0], 200.0, 100.0, 1, "", None);
        assert!(!svg.contains("<rect"));
        assert!(svg.contains("class=\"baseline\""));
        assert!(
            !svg.contains("class=\"value\""),
            "no peak, so no peak label"
        );
    }

    #[test]
    fn a_ribbon_shares_the_ceiling_it_is_given() {
        // The property the whole page rests on: rescaled per row, a person who
        // sent four messages and one who sent four hundred both draw full
        // height.
        let quiet = ribbon(&series(&[4]), 100.0, 100.0, Some(400), false, "");
        let loud = ribbon(&series(&[400]), 100.0, 100.0, Some(400), false, "");
        assert!(quiet.contains("height=\"1.5\""), "{quiet}");
        assert!(loud.contains("height=\"100\""), "{loud}");
    }

    #[test]
    fn a_ribbon_carries_no_tooltip_when_asked_not_to() {
        let svg = ribbon(&series(&[1, 2]), 100.0, 20.0, None, false, "mini");
        assert!(!svg.contains("data-tip"));
        assert!(svg.contains("class=\"chart ribbon mini\""));
    }

    #[test]
    fn a_year_axis_replaces_months_past_the_point_they_collide() {
        let long = series(&[1; 1200]);
        let svg = time_axis(&long, 1120.0, 16.0);
        assert!(svg.contains(">2025<"), "{svg}");
        assert!(svg.contains(">2028<"), "{svg}");
        assert!(!svg.contains("Jan 25"));
    }

    #[test]
    fn a_short_axis_labels_months_and_names_the_year_once() {
        let svg = time_axis(&series(&[1; 90]), 1120.0, 16.0);
        assert!(
            svg.contains(">Jan 25<"),
            "first mark carries the year: {svg}"
        );
        assert!(svg.contains(">Feb<"));
        assert!(svg.contains(">Mar<"));
    }

    #[test]
    fn the_calendar_gives_a_silent_day_the_track_and_not_the_palest_step() {
        let svg = calendar(&series(&[0, 5, 0]), 400.0, None, None);
        assert!(svg.contains("fill=\"var(--track)\""));
        assert!(svg.contains("data-tip=\"2025-01-01 &#183; 0\""));
    }

    #[test]
    fn a_matrix_greys_its_own_diagonal_rather_than_letting_it_set_the_scale() {
        let labels: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let svg = matrix(&labels, &[vec![900, 1], vec![2, 900]], 400.0, 176.0);
        assert_eq!(svg.matches("var(--self)").count(), 2);
        // 900 on the diagonal must not have set the top: the off-diagonal
        // values are 1 and 2, so 2 is the maximum shade.
        assert!(svg.contains("&#8594;"));
    }

    #[test]
    fn an_event_rail_with_no_events_draws_nothing_at_all() {
        assert_eq!(event_rail(&[], &series(&[1, 2]), 100.0, 34.0), "");
    }

    #[test]
    fn a_spanning_event_draws_a_bar_and_a_moment_draws_only_a_marker() {
        let day = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        let base = Event {
            id: "e0".into(),
            start: day(2025, 1, 2),
            end: None,
            title: "x".into(),
            summary: String::new(),
            kind: "milestone".into(),
            topic: None,
            messages: vec![],
            confidence: String::new(),
            time: String::new(),
            weight: String::new(),
            who: vec![],
            tags: vec![],
        };
        let moment = event_rail(std::slice::from_ref(&base), &series(&[1; 10]), 100.0, 34.0);
        assert!(!moment.contains("ev-span"));
        assert!(moment.contains("ev-dot"));

        let span = Event {
            end: Some(day(2025, 1, 6)),
            ..base
        };
        let svg = event_rail(&[span], &series(&[1; 10]), 100.0, 34.0);
        assert!(svg.contains("ev-span"));
    }

    #[test]
    fn low_confidence_is_rendered_differently_rather_than_hidden() {
        let event = Event {
            id: "e0".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            end: None,
            title: "x".into(),
            summary: String::new(),
            kind: "milestone".into(),
            topic: None,
            messages: vec![],
            confidence: "low".into(),
            time: String::new(),
            weight: String::new(),
            who: vec![],
            tags: vec![],
        };
        let svg = event_rail(&[event], &series(&[1; 10]), 100.0, 34.0);
        assert!(svg.contains("class=\"ev conf-low\""), "{svg}");
    }

    #[test]
    fn a_node_carries_its_count_as_area_not_as_radius() {
        // Four times the messages is twice the radius, which is four times the
        // ink for four times the data.
        let names = HashMap::new();
        let node = |key: &str, size: f64| Node {
            key: key.into(),
            x: 0.5,
            y: 0.5,
            size,
            messages: 10,
            degree: 2,
        };
        let svg = network(
            &[node("a", 1.0), node("b", 0.25)],
            &[],
            &names,
            400.0,
            400.0,
        );
        assert!(svg.contains("r=\"17\""), "{svg}");
        assert!(svg.contains("r=\"10.5\""), "{svg}");
    }

    #[test]
    fn a_bar_row_falls_back_to_its_own_number_when_given_no_note() {
        let rows = vec![("Ana".to_string(), 12.0, String::new())];
        let svg = bars_h(&rows, 400.0, 22.0, 150.0, 64.0);
        assert!(svg.contains(">12<"), "{svg}");
    }
}
