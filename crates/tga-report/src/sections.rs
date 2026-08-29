//! The eleven sections of the report, and the small helpers they share.
//!
//! **The structure is one shared time axis.** The archive's whole life is drawn
//! once, full width, at the top; every topic and every person below is drawn as
//! a smaller ribbon on *that same axis with that same ceiling*. Nothing is
//! rescaled to fill its own row, because the moment a row is rescaled you can
//! no longer read two rows against each other — and reading them against each
//! other is the entire question. It costs the quiet rows their detail. That is
//! the trade, and it is stated in the notes rather than hidden.
//!
//! Every `&str` that came from the export goes through [`esc`] on the way in.
//! A report opens as a local file, so anything that survives as markup runs
//! with that origin, and every name, filename, emoji and link in here came from
//! strangers on the internet. The same applies to the events file, which is
//! written outside the program entirely.

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::NaiveDate;
use tga_notes::{Event, Notes};
use tga_stats::{Activity, Figure, Stats};

use crate::charts::{self, esc, thousands, Day, Quantiles};
use crate::stats::{densify, every_count, human_bytes, number};

/// The viewBox width every chart on the page is drawn in.
pub const WIDTH: f64 = 1120.0;
/// How many people get a row before the rest go behind the switch.
const PEOPLE_SHOWN: usize = 30;
/// The matrix is unreadable past this, and the network diagram says the rest.
const MATRIX_PEOPLE: usize = 16;

/// Peer key -> display name, for the two charts that label by key.
pub type Names = HashMap<String, String>;

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

/// `2025-09-05` or `2025-09-05T14:30:00` -> `5 Sep 2025`.
///
/// Anything it cannot parse comes back unchanged. The alternative — raising —
/// would cost the whole report over one malformed timestamp in one row.
pub fn pretty_date(iso: &str) -> String {
    let head: String = iso.chars().take(10).collect();
    match NaiveDate::parse_from_str(&head, "%Y-%m-%d") {
        Ok(day) => {
            let text = day.format("%d %b %Y").to_string();
            text.trim_start_matches('0').to_string()
        }
        Err(_) => iso.to_string(),
    }
}

/// Seconds, in the largest unit that keeps the number readable.
pub fn duration(seconds: f64) -> String {
    let seconds = seconds.trunc() as i64;
    if seconds < 90 {
        return format!("{seconds} s");
    }
    if seconds < 90 * 60 {
        return format!("{:.0} min", seconds as f64 / 60.0);
    }
    if seconds < 36 * 3600 {
        return format!("{:.1} h", seconds as f64 / 3600.0);
    }
    format!("{:.1} days", seconds as f64 / 86400.0)
}

/// Trim a name for a narrow table cell, full string on the title.
///
/// A 36-character display name in a 90px column wraps to five lines and triples
/// the height of the row it is in.
fn short(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return esc(text);
    }
    let head: String = text.chars().take(limit - 1).collect();
    format!("<span title=\"{}\">{}&#8230;</span>", esc(text), esc(&head))
}

fn pct(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

/// A section rule: micro-heading left, a count right.
///
/// `count` is markup, not text — it carries entities like `&#183;` — so every
/// caller escapes its own interpolations. `title` is escaped here.
fn head(title: &str, count: &str, anchor: &str) -> String {
    let tag = if anchor.is_empty() {
        String::new()
    } else {
        format!(" id=\"{}\"", esc(anchor))
    };
    format!(
        "<section{tag}><div class=\"sechead\"><h2>{}</h2><span class=\"count\">{count}</span></div>",
        esc(title)
    )
}

/// `(value markup, label)` — the value is markup so a caller can put an `<em>`
/// or an entity in it; the label is escaped here.
fn figures(items: &[(String, &str)]) -> String {
    let cells: String = items
        .iter()
        .map(|(value, label)| format!("<li><b>{value}</b><span>{}</span></li>", esc(label)))
        .collect();
    format!("<ul class=\"figures\">{cells}</ul>")
}

/// `(name, numeric)` per column; cells are markup and escape their own input.
fn table(headers: &[(&str, bool)], rows: &[Vec<String>]) -> String {
    let head: String = headers
        .iter()
        .map(|(name, numeric)| {
            if *numeric {
                format!("<th class=\"n\">{}</th>", esc(name))
            } else {
                format!("<th>{}</th>", esc(name))
            }
        })
        .collect();
    let body: String = rows
        .iter()
        .map(|row| {
            let cells: String = row
                .iter()
                .zip(headers.iter())
                .map(|(cell, (_, numeric))| {
                    if *numeric {
                        format!("<td class=\"n\">{cell}</td>")
                    } else {
                        format!("<td>{cell}</td>")
                    }
                })
                .collect();
            format!("<tr>{cells}</tr>")
        })
        .collect();
    format!("<table class=\"\"><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>")
}

/// Every chart's honest twin: the numbers, as text, one click away.
fn data_view(label: &str, table: &str) -> String {
    format!(
        "<details class=\"data\"><summary>{}</summary>{table}</details>",
        esc(label)
    )
}

fn plural(count: i64, singular: &str, plural: &str) -> &'static str {
    // Returns one of two `'static` words rather than a formatted string,
    // because every call site is inside a `format!` already.
    let _ = (singular, plural);
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// An archive with no dated messages in it.
///
/// Checked rather than inferred from an empty series, because the two are
/// different facts: a dump recorded from an export that had nothing in it says
/// so, and the sections that cannot draw anything say so back.
fn is_empty_activity(activity: &Activity) -> bool {
    activity.empty
}

// ---------------------------------------------------------------------------
// masthead
// ---------------------------------------------------------------------------

pub fn masthead(stats: &Stats) -> String {
    let act = &stats.activity;
    let info = &stats.export;
    let span = if is_empty_activity(act) {
        "no dated messages".to_string()
    } else {
        format!(
            "{} &ndash; {}",
            pretty_date(&act.first),
            pretty_date(&act.last)
        )
    };
    let mut links = vec![
        ("whole", "Timeline"),
        ("rhythm", "Rhythm"),
        ("people", "People"),
        ("talk", "Conversation"),
        ("said", "What was said"),
        ("topics", "Topics"),
        ("records", "Records"),
        ("notes", "Notes"),
    ];
    // Inserted rather than appended, so the nav reads in page order. Gated on
    // the branch for the same reason the section is: a `--from-stats` dump
    // recorded before `dynamics` existed would otherwise get a nav entry that
    // scrolls nowhere.
    if stats.dynamics.is_some() {
        links.insert(4, ("between", "Between people"));
    }
    let toc: String = links
        .iter()
        .map(|(anchor, title)| format!("<a href=\"#{anchor}\">{}</a>", esc(title)))
        .collect();
    let topics = info.topics as i64;
    // The search box is the one piece of `_timeline`'s data surface this report
    // did not already have. It is a progressive enhancement by construction —
    // everything it filters is rendered server-side and visible before it is
    // typed in, so with scripting off the box does nothing and the page is
    // whole. `type="search"` rather than `text`, so the platform gives it a
    // clear control and the Escape key.
    let find = "<input class=\"find\" id=\"find\" type=\"search\" autocomplete=\"off\" \
                placeholder=\"Search people, topics, events\" \
                aria-label=\"Search people, topics, events\">\
                <span class=\"found\" id=\"found\" role=\"status\"></span>";
    format!(
        "<header class=\"masthead\">\
         <p class=\"eyebrow\">Telegram archive</p>\
         <h1>{}</h1>\
         <p class=\"lede\">{} messages across {topics} topic{}, {span}.</p>\
         <nav class=\"toc\">{toc}</nav>\
         <div class=\"switches\">\
         <button class=\"switch\" id=\"aliases\" aria-pressed=\"false\">Former names</button>\
         <button class=\"switch\" id=\"everyone\" aria-pressed=\"false\">Everyone</button>\
         {find}</div></header>",
        esc(&info.name),
        thousands(info.messages as i64),
        plural(topics, "", "s"),
    )
}

// ---------------------------------------------------------------------------
// timeline
// ---------------------------------------------------------------------------

pub fn timeline(stats: &Stats, notes: &Notes, names: &Names) -> String {
    let events = &notes.events[..];
    let events_note = notes.source.as_str();
    let act = &stats.activity;
    if is_empty_activity(act) {
        return head("Timeline", "", "whole") + "<p>No dated messages.</p></section>";
    }
    let series = &act.per_day;
    let (buckets, per) = charts::bucket_days(series, WIDTH);
    let top = buckets.iter().map(|(_, _, c)| *c).max().unwrap_or(1);

    let kinds: Vec<String> = notes.kinds();
    let rail = charts::rail(events, series, WIDTH, 34.0, &kinds);
    let ribbon = charts::ribbon(series, WIDTH, 132.0, Some(top), true, "hero");
    let axis = charts::time_axis(series, WIDTH, 16.0);

    let grain = if per == 1 {
        "day".to_string()
    } else {
        format!("{per}-day block")
    };
    let caption = format!(
        "<p class=\"caption\">One bar per {grain}; the tallest is {} messages. \
         Every row further down this page runs left to right over this same span, \
         so the same horizontal position is the same week wherever you find it.</p>",
        thousands(top)
    );

    let counts = &stats.people;
    let talk = &stats.conversation;
    let content = &stats.content;
    let stat_line = figures(&[
        (thousands(act.span_days as i64), "days spanned"),
        (thousands(act.active_days as i64), "days with messages"),
        (thousands(counts.speakers as i64), "people spoke"),
        (thousands(content.media_messages), "attachments"),
        (thousands(counts.votes_total), "reactions"),
        (thousands(talk.replies), "replies"),
    ]);

    let (listing, note) = if events.is_empty() {
        (
            String::new(),
            "<p class=\"note\">No events file yet. The ribbon shows how much was \
             said; it cannot show what any of it meant. Run the analyser with \
             <span class=\"num\">Write the digest</span> switched on, hand the \
             resulting <span class=\"num\">analysis/digest.jsonl</span> and \
             <span class=\"num\">analysis/EVENTS.md</span> to a model or a \
             person, and drop their <span class=\"num\">events.json</span> beside \
             the export. It will be pinned to this axis on the next run.</p>"
                .to_string(),
        )
    } else {
        let mut cards = String::new();
        for event in events {
            let mut when = pretty_date(&event.start.to_string());
            if event.spans() {
                if let Some(end) = event.end {
                    let _ = write!(when, " &ndash; {}", pretty_date(&end.to_string()));
                }
            }
            let cites = if event.messages.is_empty() {
                "<p class=\"cite\">no messages cited</p>".to_string()
            } else {
                let count = event.messages.len();
                let ids: Vec<String> = event
                    .messages
                    .iter()
                    .take(8)
                    .map(|m| m.to_string())
                    .collect();
                format!(
                    "<p class=\"cite\">{count} message{} cited &#183; {}{}</p>",
                    plural(count as i64, "", "s"),
                    esc(&ids.join(", ")),
                    if count > 8 { "&#8230;" } else { "" }
                )
            };
            let kind = if event.kind.is_empty() {
                String::new()
            } else {
                format!("<span class=\"kind\">{}</span> &#183; ", esc(&event.kind))
            };
            let conf = if event.confidence.is_empty() {
                String::new()
            } else {
                format!(" &#183; {} confidence", esc(&event.confidence))
            };
            let summary = {
                let escaped = esc(&event.summary);
                if escaped.is_empty() {
                    "No summary.".to_string()
                } else {
                    escaped
                }
            };
            // Everything below is a field only the `_timeline` notes layout
            // carries, so none of it appears unless the file actually said it.
            let shape = kinds.iter().position(|k| *k == event.kind).unwrap_or(0);
            if !event.time.is_empty() {
                let _ = write!(when, " <span class=\"at\">{}</span>", esc(&event.time));
            }
            let weight = if event.weight.is_empty() {
                String::new()
            } else {
                format!(
                    " <span class=\"w w-{}\">{}</span>",
                    esc(&event.weight),
                    esc(&event.weight)
                )
            };
            let tags = if event.tags.is_empty() {
                String::new()
            } else {
                format!(
                    "<p class=\"tags\">{}</p>",
                    event
                        .tags
                        .iter()
                        .map(|t| format!("<span>{}</span>", esc(t)))
                        .collect::<Vec<_>>()
                        .join("")
                )
            };
            let _ = write!(
                cards,
                "<li class=\"k{shape}\" id=\"event-{}\" data-kind=\"{}\"><time>{when}</time>\
                 <div><h4>{}{weight}</h4><p>{kind}{summary}{conf}</p>{}{tags}{cites}</div></li>",
                esc(&event.id),
                esc(&event.kind),
                esc(&event.title),
                who_line(&event.who, names)
            );
        }
        (
            format!(
                "{}<ul class=\"events\">{cards}</ul>",
                kind_filters(&kinds, events)
            ),
            format!(
                "<p class=\"note\">Events read from {}.</p>",
                esc(events_note)
            ),
        )
    };

    let months: Vec<Vec<String>> = act
        .per_month
        .iter()
        .map(|month| vec![esc(&month.label), thousands(month.n)])
        .collect();

    let coverage = coverage_panel(notes.coverage.as_ref(), series);

    format!(
        "{}{rail}{coverage}{ribbon}{axis}{caption}{stat_line}{note}{listing}{}</section>",
        head(
            "Timeline",
            &format!("{} days", thousands(act.span_days as i64)),
            "whole"
        ),
        data_view(
            "Messages per month",
            &table(&[("Month", false), ("Messages", true)], &months)
        )
    )
}

/// What the notes claim to have read, drawn against the whole archive.
///
/// Renders nothing when the file states no coverage — an absent claim and a
/// claim of nothing are different, and inventing "covers everything" from
/// silence is exactly the reading this block exists to prevent.
fn coverage_panel(cover: Option<&tga_notes::Coverage>, series: &[Day]) -> String {
    let Some(cover) = cover else {
        return String::new();
    };
    let band = charts::coverage_band(cover.from, cover.to, series, WIDTH, 6.0);

    let span = match (cover.from, cover.to) {
        (Some(from), Some(to)) => format!(
            "{} &ndash; {}",
            pretty_date(&from.to_string()),
            pretty_date(&to.to_string())
        ),
        (Some(from), None) => format!("from {}", pretty_date(&from.to_string())),
        (None, Some(to)) => format!("up to {}", pretty_date(&to.to_string())),
        (None, None) => String::new(),
    };

    // The share of the archive's days the claim covers, because "one month of
    // eleven" is the fact and "5 Sep – 5 Oct" only implies it.
    let share = match (cover.from, cover.to, series.first(), series.last()) {
        (Some(from), Some(to), Some(first), Some(last)) => {
            let whole = (iso_day(&last.label) - iso_day(&first.label))
                .num_days()
                .max(1);
            let read = (to - from).num_days().max(0) + 1;
            format!(
                " &#183; {} of {} days",
                thousands(read.min(whole + 1)),
                thousands(whole + 1)
            )
        }
        _ => String::new(),
    };

    let level = if cover.level.is_empty() {
        String::new()
    } else {
        format!(" &#183; {}", esc(&cover.level))
    };
    let note = if cover.note.is_empty() {
        String::new()
    } else {
        format!("<p class=\"caption\">{}</p>", esc(&cover.note))
    };

    format!("{band}<p class=\"coverage\"><b>Read</b> {span}{share}{level}</p>{note}")
}

fn iso_day(text: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap_or_default()
}

/// One toggle per kind actually present, shape-coded to match the rail.
///
/// Built from the kinds the *file* used rather than from a fixed vocabulary, so
/// a file using three of four gets three controls and a file using a word
/// nobody anticipated still gets one. The glyph is the same shape the marker
/// draws, and the word is beside it — nothing rests on telling ● from ■ alone.
fn kind_filters(kinds: &[String], events: &[Event]) -> String {
    if kinds.len() < 2 {
        // One kind is not a choice, and a filter row that can only be all-on or
        // all-off is a control that does nothing useful.
        return String::new();
    }
    const GLYPHS: [&str; 4] = ["\u{25cf}", "\u{25a0}", "\u{25b2}", "\u{25c6}"];
    let cells: String = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let count = events.iter().filter(|e| e.kind == *kind).count();
            format!(
                "<button class=\"kind-filter on\" data-k=\"{index}\" aria-pressed=\"true\">\
                 <i class=\"g{}\">{}</i>{} <span>{count}</span></button>",
                index % GLYPHS.len(),
                GLYPHS[index % GLYPHS.len()],
                esc(kind)
            )
        })
        .collect();
    format!("<div class=\"kinds\">{cells}</div>")
}

/// The names an event credits, each checked against the statistics.
///
/// **A `who` that names nobody says so.** PLAN.md asks for this by name, and
/// the reason is practical rather than pedantic: a name in the notes that
/// matches no one in the export almost always means the export spells it
/// differently — somebody renamed themselves, or the writer used the name they
/// know them by. That is a thing the reader can go and fix, and it is invisible
/// unless the report points at it. Reporting it must not break the panel, so an
/// unmatched name is still shown, just marked.
fn who_line(who: &[String], names: &Names) -> String {
    if who.is_empty() {
        return String::new();
    }
    // Case- and space-insensitive, because the notes are typed by hand and the
    // export's spelling is whatever somebody set as their display name.
    let known: std::collections::HashSet<String> = names
        .values()
        .map(|n| {
            n.to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let mut missing = 0usize;
    let cells: String = who
        .iter()
        .map(|name| {
            let key = name
                .to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if known.contains(&key) {
                format!("<span>{}</span>", esc(name))
            } else {
                missing += 1;
                format!("<span class=\"unknown\">{}</span>", esc(name))
            }
        })
        .collect();
    let note = if missing == 0 {
        String::new()
    } else {
        format!(
            " <em>{missing} name{} match{} nobody in this export</em>",
            plural(missing as i64, "", "s"),
            if missing == 1 { "es" } else { "" }
        )
    };
    format!("<p class=\"who\">{cells}{note}</p>")
}

// ---------------------------------------------------------------------------
// rhythm
// ---------------------------------------------------------------------------

pub fn rhythm(stats: &Stats) -> String {
    let act = &stats.activity;
    if is_empty_activity(act) {
        return String::new();
    }
    let hours: Vec<String> = (0..24).map(|h| format!("{h:02}")).collect();
    let weekdays: Vec<String> = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let per_hour = act.per_hour;
    let per_weekday = act.per_weekday;
    let hour_weekday: Vec<Vec<i64>> = act.hour_weekday.iter().map(|row| row.to_vec()).collect();
    let series = &act.per_day;

    let hour_chart = charts::columns(
        &hours,
        &per_hour,
        WIDTH / 2.0 - 28.0,
        190.0,
        3,
        " messages",
        None,
    );
    let day_chart = charts::columns(
        &weekdays,
        &per_weekday,
        WIDTH / 2.0 - 28.0,
        190.0,
        1,
        " messages",
        None,
    );
    let grid_scale = Quantiles::new(hour_weekday.iter().flatten().copied());
    let heat = charts::heatgrid(
        &weekdays,
        &hours,
        &hour_weekday,
        WIDTH,
        28.0,
        46.0,
        Some(&grid_scale),
    );
    let cal_scale = Quantiles::new(series.iter().map(|day| day.n));
    let cal = charts::calendar(series, WIDTH, None, Some(&cal_scale));

    let full = [
        "Mondays",
        "Tuesdays",
        "Wednesdays",
        "Thursdays",
        "Fridays",
        "Saturdays",
        "Sundays",
    ];
    let peak_hour = argmax(&per_hour);
    let peak_day = full[argmax(&per_weekday).min(6)];

    let hour_rows: Vec<Vec<String>> = per_hour
        .iter()
        .enumerate()
        .map(|(h, n)| vec![format!("{h:02}:00"), thousands(*n)])
        .collect();

    format!(
        "{}<div class=\"cols2\"><div><h3>Hour of day</h3>{hour_chart}\
         <p class=\"caption\">Local time, as the export recorded it.</p></div>\
         <div><h3>Day of week</h3>{day_chart}\
         <p class=\"caption\">Every message, across the whole archive.</p></div></div>\
         <h3>Both at once</h3><div class=\"split\"><div>{heat}</div><aside>{}\
         <p class=\"caption\">Each cell is one hour of one weekday, summed over \
         the whole archive. An empty cell is an hour in which nothing was ever \
         said. {}</p></aside></div>\
         <h3>Every day</h3><div class=\"split\"><div>{cal}</div><aside>{}\
         <p class=\"caption\">One square per day. {}</p></aside></div>{}</section>",
        head(
            "Rhythm",
            &format!("busiest at {peak_hour:02}:00, on {peak_day}"),
            "rhythm"
        ),
        charts::legend(5, "quiet", "busy"),
        esc(&grid_scale.caption("messages")),
        charts::legend(5, "silent", "busiest"),
        esc(&cal_scale.caption("messages")),
        data_view(
            "Hour of day, as numbers",
            &table(&[("Hour", false), ("Messages", true)], &hour_rows)
        )
    )
}

/// The index of the first maximum.
fn argmax(values: &[i64]) -> usize {
    let top = values.iter().max().copied().unwrap_or(0);
    values.iter().position(|v| *v == top).unwrap_or(0)
}

/// One presence row.
///
/// See [`charts::strip_compact`] for what the compact form gives up and why it
/// is the whole size budget — these rows are 64% of a large report drawn the
/// obvious way.
pub const PRESENCE_WIDTH: f64 = WIDTH * 0.34;
const PRESENCE_HEIGHT: f64 = 15.0;

fn presence(dense: &[Day], scale: &Quantiles) -> String {
    charts::strip_compact(dense, PRESENCE_WIDTH, PRESENCE_HEIGHT, scale, "")
}

// ---------------------------------------------------------------------------
// people
// ---------------------------------------------------------------------------

pub fn people(stats: &Stats) -> String {
    let folk = &stats.people;
    let act = &stats.activity;
    let rows: Vec<&tga_stats::Person> = folk.rows.iter().filter(|r| r.messages != 0).collect();
    if rows.is_empty() {
        return String::new();
    }

    let series = &act.per_day;
    let per_person = &act.per_day_by_person;
    // One scale for every row, built from every person-day in the archive.
    // Shared, so a shade means the same thing in row 1 and row 30.
    let scale = Quantiles::new(every_count(per_person));

    let mut body = String::new();
    for (index, row) in rows.iter().enumerate() {
        let dense = densify(per_person.get(&row.key), series);
        let mini = presence(&dense, &scale);
        let alias = if row.aliases.is_empty() {
            String::new()
        } else {
            format!(
                " <span class=\"alias\">was {}</span>",
                esc(&row.aliases.join(", "))
            )
        };
        let role = if row.role.is_empty() || row.role == "member" {
            String::new()
        } else {
            format!(" <span class=\"role\">{}</span>", esc(&row.role))
        };
        let cls = if index >= PEOPLE_SHOWN {
            " class=\"overflow\""
        } else {
            ""
        };
        let _ = write!(
            body,
            "<tr{cls}><td class=\"rank\">{}</td>\
             <td class=\"name\">{}{role}{alias}</td>\
             <td class=\"presence\">{mini}</td>\
             <td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td>\
             <td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
            index + 1,
            esc(&row.name),
            thousands(row.messages),
            pct(row.share),
            number(row.avg_words),
            thousands(row.reactions_received),
            esc(&pretty_date(row.first.as_deref().unwrap_or_default())),
            esc(&pretty_date(row.last.as_deref().unwrap_or_default())),
        );
    }

    let headers = "<tr><th></th><th>Name</th><th>Presence</th>\
                   <th class=\"n\">Messages</th><th class=\"n\">Share</th>\
                   <th class=\"n\">Words/msg</th><th class=\"n\">Reactions</th>\
                   <th class=\"n\">First seen</th><th class=\"n\">Last seen</th></tr>";
    let listing = format!("<table><thead>{headers}</thead><tbody>{body}</tbody></table>");

    let hidden = rows.len().saturating_sub(PEOPLE_SHOWN);
    let more = if hidden == 0 {
        String::new()
    } else {
        format!(
            "<p class=\"note\">{hidden} more {} sent at least one message. \
             Use <span class=\"num\">Everyone</span> above to show them.</p>",
            if hidden == 1 { "person" } else { "people" }
        )
    };

    let mut silent = String::new();
    if folk.silent_members != 0 {
        silent = format!(
            "<p class=\"note\">{} of the {} people on the member list never posted, \
             so they appear nowhere above.</p>",
            folk.silent_members, folk.known_members
        );
    }
    if folk.roster_complete == Some(false) {
        silent.push_str(
            "<p class=\"note\">The member list in this export is incomplete, \
             so the count of people who never posted is a floor.</p>",
        );
    }

    let awards = if stats.awards.is_empty() {
        String::new()
    } else {
        let cells: String = stats
            .awards
            .iter()
            .map(|a| {
                format!(
                    "<li><b>{}</b><span>{}: {}</span></li>",
                    esc(&a.value),
                    esc(&a.title),
                    esc(&a.name)
                )
            })
            .collect();
        format!(
            "<h3>Who does what</h3><ul class=\"figures\">{cells}</ul>\
             <p class=\"caption\">Restricted to people with 40 messages or more; \
             below that every one of these is won by somebody who posted four \
             times, all of them at 4am.</p>"
        )
    };

    let streak = match &stats.streak {
        None => String::new(),
        Some(run) => format!(
            "<p class=\"note\">Longest unbroken run of days posted on: {}, {} days to {}.</p>",
            esc(&run.name),
            run.days,
            esc(&pretty_date(&run.ended))
        ),
    };

    let votes = format!(
        "<p class=\"note\">Reactions given are a floor, not a total: Telegram \
         names at most three reactors per message and never names an anonymous \
         one. {} of {} reactions in this archive have a name on them.</p>",
        thousands(folk.votes_named),
        thousands(folk.votes_total)
    );

    let known = folk.known_members;
    let count = format!("{} spoke", folk.speakers)
        + &if known != 0 {
            format!(" &#183; {known} on the member list")
        } else {
            String::new()
        };

    format!(
        "{}<p class=\"lede\">The three loudest carry {} of everything said.</p>\
         <p class=\"caption\">Presence runs on the same time axis as the ribbon \
         at the top of the page, and its shading is on one scale shared by \
         every row. {}</p>{listing}{more}{silent}{streak}{awards}{votes}</section>",
        head("People", &count, "people"),
        pct(folk.top3_share),
        esc(&scale.caption("messages in a day"))
    )
}

// ---------------------------------------------------------------------------
// conversation
// ---------------------------------------------------------------------------

pub fn conversation(stats: &Stats, names: &Names) -> String {
    let talk = &stats.conversation;
    let stat_line = figures(&[
        (thousands(talk.replies), "replies"),
        (pct(talk.reply_share), "of messages are replies"),
        (
            esc(&duration(talk.latency_median as f64)),
            "median time to reply",
        ),
        (esc(&duration(talk.latency_p90 as f64)), "90th percentile"),
        (thousands(talk.sessions as i64), "bursts of talk"),
        (
            thousands(talk.session_median_messages),
            "messages in a typical burst",
        ),
    ]);

    let fast = if talk.fastest.is_empty() {
        String::new()
    } else {
        // Slowest first, so the bar length is the wait itself. Inverting it to
        // put the fastest on the longest bar reads better and lies: the mark
        // would no longer be the number printed beside it.
        let slowest: Vec<&tga_stats::Latency> = talk.fastest.iter().take(12).rev().collect();
        let rows: Vec<charts::BarRow> = slowest
            .iter()
            .map(|r| {
                let median = r.median as f64;
                (r.name.clone(), median, duration(median))
            })
            .collect();
        let bars = charts::bars_h(&rows, WIDTH, 22.0, 250.0, 64.0);
        let quickest = slowest[slowest.len() - 1];
        format!(
            "<h3>How long people wait before answering</h3>{bars}\
             <p class=\"caption\">Median gap between a message and their reply to it, \
             for anyone with five replies or more. Shorter is faster; {} is quickest \
             at {}.</p>",
            esc(&quickest.name),
            esc(&duration(quickest.median as f64))
        )
    };

    let edges = &talk.edges;
    let matrix_html = if edges.is_empty() {
        String::new()
    } else {
        // Insertion-ordered, because the sort below is stable and first-seen
        // order is what breaks a tie on volume.
        let mut order: Vec<String> = Vec::new();
        let mut volume: HashMap<String, i64> = HashMap::new();
        let mut bump = |key: &str, count: i64, order: &mut Vec<String>| {
            let entry = volume.entry(key.to_string()).or_insert_with(|| {
                order.push(key.to_string());
                0
            });
            *entry += count;
        };
        for edge in edges {
            bump(&edge.from, edge.count, &mut order);
            bump(&edge.to, edge.count, &mut order);
        }
        let mut keys = order.clone();
        keys.sort_by_key(|k| std::cmp::Reverse(volume[k]));
        keys.truncate(MATRIX_PEOPLE);

        let index: HashMap<&str, usize> = keys
            .iter()
            .enumerate()
            .map(|(n, k)| (k.as_str(), n))
            .collect();
        let mut cells = vec![vec![0i64; keys.len()]; keys.len()];
        for edge in edges {
            if let (Some(&from), Some(&to)) =
                (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
            {
                cells[from][to] += edge.count;
            }
        }
        let labels: Vec<String> = keys.iter().map(|k| label_for(names, k)).collect();
        format!(
            "<h3>Who answers whom</h3><div class=\"split\"><div>{}</div><aside>{}\
             <p class=\"caption\">A row replies; a column is replied to. The \
             diagonal is greyed out &#8212; everyone answers themselves, and \
             counting it would make the loudest person their own closest \
             correspondent.</p></aside></div>",
            charts::matrix(&labels, &cells, WIDTH * 0.66, 176.0),
            charts::legend(5, "never", "most often")
        )
    };

    let net = &stats.graph;
    let network_html = if net.nodes.is_empty() {
        String::new()
    } else {
        let drawn: Vec<charts::Node> = net
            .nodes
            .iter()
            .map(|n| charts::Node {
                key: n.key.clone(),
                x: n.x,
                y: n.y,
                size: n.size,
                messages: n.messages,
                degree: n.degree,
            })
            .collect();
        let links: Vec<charts::Link> = net
            .edges
            .iter()
            .map(|e| charts::Link {
                a: e.a.clone(),
                b: e.b.clone(),
                weight: e.weight,
            })
            .collect();
        let labels: Names = drawn
            .iter()
            .map(|n| (n.key.clone(), label_for(names, &n.key)))
            .collect();
        let hidden = net.hidden;
        format!(
            "<h3>Who is in contact with whom</h3>{}\
             <p class=\"caption\">Replies and reactions pooled and drawn \
             undirected: an edge means these two are in contact, and its weight \
             is how often. Circle area is how much that person posted. {}</p>",
            charts::network(&drawn, &links, &labels, WIDTH, 620.0),
            if hidden != 0 {
                format!(
                    "The {hidden} least-connected people are left out; the \
                     matrix above has the numbers."
                )
            } else {
                String::new()
            }
        )
    };

    let starters = if talk.starters.is_empty() {
        String::new()
    } else {
        let rows: Vec<charts::BarRow> = talk
            .starters
            .iter()
            .take(10)
            .map(|st| (st.name.clone(), st.count as f64, thousands(st.count)))
            .collect();
        format!(
            "<h3>Who breaks the silence</h3>{}\
             <p class=\"caption\">First message after a gap of half an hour or more.</p>",
            charts::bars_h(&rows, WIDTH, 22.0, 210.0, 64.0)
        )
    };

    let orphans = talk.orphan_replies;
    let orphan = if orphans == 0 {
        String::new()
    } else {
        format!(
            "<p class=\"note\">{orphans} replies point at a message that is not \
             in this export, so they have no target in any figure above.</p>"
        )
    };

    format!(
        "{}<p class=\"lede\">A burst is talk with no gap longer than half an \
         hour, counted per topic &#8212; two topics running at once are two \
         conversations.</p>{stat_line}{fast}{starters}{matrix_html}{network_html}{orphan}</section>",
        head(
            "Conversation",
            &format!("{} bursts", thousands(talk.sessions as i64)),
            "talk"
        )
    )
}

/// The display name for a peer key.
///
/// Falls back to the key itself, which is what `People::get` hands back for a
/// key it does not know. A key on screen is ugly and it is *true*; a blank cell
/// silently drops a person out of the chart.
fn label_for(names: &Names, key: &str) -> String {
    names.get(key).cloned().unwrap_or_else(|| {
        if key.is_empty() {
            "unknown".to_string()
        } else {
            key.to_string()
        }
    })
}

// ---------------------------------------------------------------------------
// between people
// ---------------------------------------------------------------------------

/// The `dynamics` branch: pairs, answer latency, tenure, retention and depth.
///
/// It renders nothing at all when the branch is absent, which is the ordinary
/// case for a `--from-stats` dump recorded before the branch existed.
pub fn between(stats: &Stats) -> String {
    let dynamics = match &stats.dynamics {
        Some(branch) if !branch.empty => branch,
        _ => return String::new(),
    };
    let (pairs_b, answer, tenure, retention, depth) = (
        &dynamics.pairs,
        &dynamics.answer,
        &dynamics.tenure,
        &dynamics.retention,
        &dynamics.depth,
    );

    // -- who talks with whom ------------------------------------------------
    let rows = &pairs_b.rows;
    let bars: Vec<charts::BarRow> = rows
        .iter()
        .map(|row| {
            (
                format!("{} & {}", row.a_name, row.b_name),
                row.both as f64,
                format!(
                    "{} \u{2194} {}",
                    thousands(row.a_to_b),
                    thousands(row.b_to_a)
                ),
            )
        })
        .collect();
    let pair_chart = if bars.is_empty() {
        String::new()
    } else {
        charts::bars_h(&bars, WIDTH, 22.0, 260.0, 96.0)
    };
    let pair_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            vec![
                short(&row.a_name, 26),
                short(&row.b_name, 26),
                thousands(row.a_to_b),
                thousands(row.b_to_a),
                pct(row.balance),
            ]
        })
        .collect();

    // -- when an answer arrives ---------------------------------------------
    let hours: Vec<String> = (0..24).map(|h| format!("{h:02}")).collect();
    let medians = answer.medians;
    let counts = answer.counts;
    let floor = answer.minimum;
    // An hour that has not met the floor is drawn as nothing rather than as a
    // short bar. A median over four replies is not a fast hour, and a bar is
    // read as one.
    let shown: Vec<i64> = medians
        .iter()
        .zip(counts.iter())
        .map(|(m, n)| if *n >= floor { *m } else { 0 })
        .collect();
    let answer_chart = if shown.iter().all(|v| *v == 0) {
        String::new()
    } else {
        charts::columns(
            &hours,
            &shown,
            WIDTH / 2.0 - 28.0,
            190.0,
            3,
            " median",
            None,
        )
    };
    let answer_rows: Vec<Vec<String>> = medians
        .iter()
        .zip(counts.iter())
        .enumerate()
        .map(|(hour, (median, n))| {
            vec![
                format!("{hour:02}:00"),
                thousands(*n),
                if *n >= floor {
                    duration(*median as f64)
                } else {
                    "&#8212;".to_string()
                },
            ]
        })
        .collect();

    // -- how far a thread runs ----------------------------------------------
    let buckets = &depth.buckets;
    let depth_chart = if buckets.is_empty() {
        String::new()
    } else {
        charts::columns(
            &buckets.iter().map(|b| b.label.clone()).collect::<Vec<_>>(),
            &buckets.iter().map(|b| b.n).collect::<Vec<_>>(),
            WIDTH / 2.0 - 28.0,
            190.0,
            1,
            " chains",
            None,
        )
    };

    // -- month over month ---------------------------------------------------
    let months = &retention.months;
    let (active, fresh, back, lost) = (
        &retention.active,
        &retention.new,
        &retention.returning,
        &retention.lost,
    );
    let label_every = (months.len() / 12).max(1);
    let ret_chart = if months.is_empty() {
        String::new()
    } else {
        charts::columns(
            &months
                .iter()
                .map(|m| m[2..].to_string())
                .collect::<Vec<_>>(),
            back,
            WIDTH,
            170.0,
            label_every,
            " came back",
            None,
        )
    };
    let ret_rows: Vec<Vec<String>> = months
        .iter()
        .enumerate()
        .map(|(at, month)| {
            let cell = |series: &[i64]| thousands(series.get(at).copied().unwrap_or(0));
            vec![
                esc(month),
                cell(active),
                cell(fresh),
                cell(back),
                cell(lost),
            ]
        })
        .collect();

    // -- who is still here --------------------------------------------------
    // Ranked by the silence rather than by the message count, because that is
    // the one ordering the People table cannot already give you.
    let mut quiet: Vec<&tga_stats::TenureRow> = tenure
        .rows
        .iter()
        .filter(|row| row.status != "active")
        .collect();
    quiet.sort_by_key(|row| std::cmp::Reverse(row.dormant_days));
    let quiet_rows: Vec<Vec<String>> = quiet
        .iter()
        .take(PEOPLE_SHOWN)
        .map(|row| {
            vec![
                short(&row.name, 30),
                thousands(row.messages),
                esc(&pretty_date(&row.last)),
                thousands(row.dormant_days),
                esc(&row.status),
            ]
        })
        .collect();
    let quiet_table = if quiet_rows.is_empty() {
        "<p class=\"caption\">Everybody with a message in this archive posted \
         one in its last month.</p>"
            .to_string()
    } else {
        table(
            &[
                ("Person", false),
                ("Messages", true),
                ("Last seen", false),
                ("Days since", true),
                ("", false),
            ],
            &quiet_rows,
        )
    };

    let counted = answer.counted;
    let stat_line = format!(
        "{} mutual, {} one-way",
        thousands(pairs_b.mutual),
        thousands(pairs_b.one_way)
    );

    let mut out = head("Between people", &stat_line, "between");
    let _ = write!(
        out,
        "{}",
        figures(&[
            (thousands(pairs_b.mutual), "pairs answering each other"),
            (thousands(tenure.active), "still posting"),
            (thousands(tenure.fading), "gone quiet"),
            (thousands(tenure.gone), "long gone"),
            (pct(retention.kept_mean), "kept month to month"),
            (thousands(depth.max), "deepest chain"),
        ])
    );

    if !pair_chart.is_empty() {
        let _ = write!(
            out,
            "<h3>Who talks with whom</h3>{pair_chart}\
             <p class=\"caption\">Ranked on the <em>smaller</em> of the two \
             directions, so a pair is only as strong as the quieter half of it. \
             The two numbers are replies each way. A pair that only ever runs \
             one way is somebody being answered, not a correspondence &#8212; \
             {} of the {} pairs here are that.</p>{}",
            thousands(pairs_b.one_way),
            thousands(pairs_b.mutual + pairs_b.one_way),
            data_view(
                "Correspondents, as numbers",
                &table(
                    &[
                        ("Person", false),
                        ("Person", false),
                        ("&rarr;", true),
                        ("&larr;", true),
                        ("Balance", true),
                    ],
                    &pair_rows,
                )
            )
        );
    }

    let _ = write!(
        out,
        "<div class=\"cols2\"><div><h3>How long until an answer</h3>{}\
         <p class=\"caption\">By the hour the message being answered was posted, \
         not the hour the answer arrived. An hour with fewer than {} replies is \
         left blank rather than drawn from too little. {} replies counted, none \
         further apart than {}.</p></div>\
         <div><h3>How far a thread runs</h3>{}\
         <p class=\"caption\">Messages by the length of the reply chain they sit \
         at the end of. A chain of two is one answer; most of the archive is a \
         chain of one and is not drawn.</p></div></div>{}",
        if answer_chart.is_empty() {
            "<p class=\"caption\">No hour has enough replies to quote a median \
             for.</p>"
                .to_string()
        } else {
            answer_chart
        },
        thousands(floor),
        thousands(counted),
        duration(answer.cap as f64),
        if depth_chart.is_empty() {
            "<p class=\"caption\">Nothing in this archive was replied to.</p>".to_string()
        } else {
            depth_chart
        },
        data_view(
            "Answer times, as numbers",
            &table(
                &[("Hour", false), ("Replies", true), ("Median", false)],
                &answer_rows,
            )
        )
    );

    if !ret_chart.is_empty() {
        let _ = write!(
            out,
            "<h3>Who came back</h3>{ret_chart}\
             <p class=\"caption\">People who spoke in a month <em>and</em> in the \
             month before it. Silent months stay on the axis: closing a gap would \
             report the first month after it as an ordinary return. Nobody is \
             announced for going quiet, which is why this is not in Coming and \
             going.</p>{}",
            data_view(
                "Month by month, as numbers",
                &table(
                    &[
                        ("Month", false),
                        ("Spoke", true),
                        ("New", true),
                        ("Returning", true),
                        ("Lost", true),
                    ],
                    &ret_rows,
                )
            )
        );
    }

    let _ = write!(
        out,
        "<h3>Gone quiet</h3>{quiet_table}\
         <p class=\"caption\">Measured against {}, the last day in this archive, \
         not against today &#8212; the same file read a year from now says the \
         same thing. Quiet past {} days, gone past {}.</p></section>",
        esc(&pretty_date(&tenure.as_of)),
        thousands(tenure.active_within),
        thousands(tenure.fading_within),
    );
    out
}

// ---------------------------------------------------------------------------
// what was said
// ---------------------------------------------------------------------------

pub fn said(stats: &Stats) -> String {
    let content = &stats.content;
    let kinds = &content.media_kinds;
    let media_chart = if kinds.is_empty() {
        String::new()
    } else {
        let rows: Vec<charts::BarRow> = kinds
            .iter()
            .map(|k| {
                (
                    k.label.clone(),
                    k.sent as f64,
                    format!("{}  {}", thousands(k.sent), human_bytes(k.bytes)),
                )
            })
            .collect();
        charts::bars_h(&rows, WIDTH, 22.0, 140.0, 64.0)
    };

    let lengths = &content.lengths;
    // Narrower than the measure on purpose. Nine buckets across the full page
    // gives 124px bands, and a column capped at 24px inside one of those reads
    // as nine unrelated marks rather than as a distribution.
    let length_chart = charts::columns(
        &lengths.iter().map(|b| b.label.clone()).collect::<Vec<_>>(),
        &lengths.iter().map(|b| b.n).collect::<Vec<_>>(),
        WIDTH * 0.56,
        210.0,
        1,
        " messages",
        None,
    );

    let symbol_rows = |items: &[tga_stats::Count], title: &str| -> String {
        if items.is_empty() {
            return String::new();
        }
        let cells: String = items
            .iter()
            .take(10)
            .map(|item| {
                format!(
                    "<li><b>{}</b><span>{}</span></li>",
                    esc(&item.label),
                    thousands(item.n)
                )
            })
            .collect();
        format!("<h3>{title}</h3><ul class=\"figures\">{cells}</ul>")
    };
    let emoji_rows = symbol_rows(&content.emoji, "Most-used emoji");
    let sticker_rows = symbol_rows(&content.stickers, "Favourite stickers");

    let domains = &content.domains;
    let links = if domains.is_empty() {
        String::new()
    } else {
        let rows: Vec<charts::BarRow> = domains
            .iter()
            .take(14)
            .map(|host| (host.label.clone(), host.n as f64, thousands(host.n)))
            .collect();
        format!(
            "<h3>Where the links went</h3>{}",
            charts::bars_h(&rows, WIDTH, 22.0, 210.0, 64.0)
        )
    };

    let mut tag_cols = String::new();
    let hashtags = &content.hashtags;
    if !hashtags.is_empty() {
        let rows: Vec<Vec<String>> = hashtags
            .iter()
            .take(12)
            .map(|tag| vec![format!("#{}", esc(&tag.label)), thousands(tag.n)])
            .collect();
        let _ = write!(
            tag_cols,
            "<div><h3>Hashtags</h3>{}</div>",
            table(&[("Tag", false), ("Uses", true)], &rows)
        );
    }
    let mentions = &content.mentions;
    if !mentions.is_empty() {
        let rows: Vec<Vec<String>> = mentions
            .iter()
            .take(12)
            .map(|who| vec![esc(&who.label), thousands(who.n)])
            .collect();
        let _ = write!(
            tag_cols,
            "<div><h3>Most mentioned</h3>{}</div>",
            table(&[("Handle", false), ("Mentions", true)], &rows)
        );
    }
    let tags = if tag_cols.is_empty() {
        String::new()
    } else {
        format!("<div class=\"cols2\">{tag_cols}</div>")
    };

    let sources = &content.forward_sources;
    let forwards = if sources.is_empty() {
        String::new()
    } else {
        let rows: Vec<Vec<String>> = sources
            .iter()
            .take(12)
            .map(|src| vec![esc(&src.label), thousands(src.n)])
            .collect();
        format!(
            "<h3>Forwarded from</h3>{}",
            table(&[("Source", false), ("Forwards", true)], &rows)
        )
    };

    let saved = content.media_saved;
    let total = content.media_messages;
    let skipped = total - saved;
    let messages = content.messages as i64;
    let stat_line = figures(&[
        (thousands(content.total_words), "words"),
        (number(content.mean_words), "words in a typical message"),
        (thousands(total), "messages with an attachment"),
        (esc(&human_bytes(content.media_bytes)), "shared"),
        (thousands(content.links_total), "links"),
        (thousands(content.edited), "edited afterwards"),
    ]);

    let skip_note = if skipped > 0 {
        format!(
            "<p class=\"note\">{} of those attachments were over the \
             export&#8217;s size limit, so they are described in the archive but their \
             bytes are not on disk. The totals above are what was shared, which \
             includes them.</p>",
            thousands(skipped)
        )
    } else {
        String::new()
    };

    let no_text = lengths
        .iter()
        .find(|bucket| bucket.label == "no text")
        .map(|bucket| bucket.n)
        .unwrap_or(0);

    let attachments = if media_chart.is_empty() {
        String::new()
    } else {
        format!("<h3>Attachments</h3>{media_chart}{skip_note}")
    };

    let kind_rows: Vec<Vec<String>> = kinds
        .iter()
        .map(|k| {
            vec![
                esc(&k.label),
                thousands(k.sent),
                thousands(k.saved),
                esc(&human_bytes(k.bytes)),
            ]
        })
        .collect();

    format!(
        "{}{stat_line}<h3>Message length</h3><div class=\"split\"><div>{length_chart}</div>\
         <aside><p class=\"caption\">In words. The first bucket is messages with no \
         text at all &#8212; a photo, a sticker, a voice note, which is {} of \
         everything sent.</p></aside></div>{attachments}{emoji_rows}{sticker_rows}\
         {links}{tags}{forwards}{}</section>",
        head(
            "What was said",
            &format!("{} messages", thousands(messages)),
            "said"
        ),
        pct(no_text as f64 / messages.max(1) as f64),
        data_view(
            "Attachments, as numbers",
            &table(
                &[
                    ("Kind", false),
                    ("Sent", true),
                    ("Saved", true),
                    ("Bytes", true)
                ],
                &kind_rows
            )
        )
    )
}

// ---------------------------------------------------------------------------
// coming and going
// ---------------------------------------------------------------------------

/// The inner markup of the churn section; [`crate::render`] wraps it.
pub fn churn(stats: &Stats) -> String {
    let churn = &stats.churn;
    let months = &churn.months;
    if months.is_empty() {
        return String::new();
    }
    let labels: Vec<String> = months.iter().map(|m| m.chars().skip(2).collect()).collect();
    let every = (months.len() / 14).max(1);

    let active = charts::columns(
        &labels,
        &churn.active.iter().map(|n| *n as i64).collect::<Vec<_>>(),
        WIDTH,
        200.0,
        every,
        " people",
        None,
    );
    let leaves: i64 = churn.announced_leaves.iter().sum();
    // No member list means no dated arrivals — a Telegram Desktop export never
    // has one, and neither does ours if the roster was not fetched. Drawing an
    // empty chart there says "nobody joined", which is a different claim from
    // "this export cannot tell you".
    let roster_joins = &churn.roster_joins;
    let dated: i64 = roster_joins.iter().sum();
    let joins = if dated != 0 {
        charts::columns(&labels, roster_joins, WIDTH, 170.0, every, " joined", None)
    } else {
        String::new()
    };

    let note = if dated != 0 {
        format!(
            "<p class=\"caption\">Arrivals are dated from the member list, which \
             knows only about people who are still in the group: {} of {} current \
             members carry a join date. Anyone who has since left is missing from \
             it entirely{}</p>",
            thousands(churn.roster_dated),
            thousands(churn.roster_size as i64),
            if leaves != 0 {
                format!(
                    ", though the history announces {} removals.",
                    thousands(leaves)
                )
            } else {
                ".".to_string()
            }
        )
    } else {
        let announced: i64 = churn.announced_joins.iter().sum();
        format!(
            "<p class=\"caption\">This export carries no member list, so arrivals \
             cannot be dated{}</p>",
            if leaves != 0 || announced != 0 {
                format!(
                    ". The history does announce {} joins and {} removals, which is \
                     only the ones Telegram posted about.",
                    thousands(announced),
                    thousands(leaves)
                )
            } else {
                ", and the history announces none.".to_string()
            }
        )
    };

    format!(
        "<h3>People talking each month</h3>{active}\
         <p class=\"caption\">Distinct people who posted at least once that month \
         &#8212; not the size of the group.</p>{}{joins}{note}",
        if joins.is_empty() {
            ""
        } else {
            "<h3>Arrivals</h3>"
        }
    )
}

// ---------------------------------------------------------------------------
// topics
// ---------------------------------------------------------------------------

pub fn topics(stats: &Stats) -> String {
    let rows = &stats.topics;
    let act = &stats.activity;
    if rows.is_empty() {
        return String::new();
    }
    let series = &act.per_day;
    let by_topic = &act.per_day_by_topic;
    let scale = Quantiles::new(every_count(by_topic));

    let mut body = String::new();
    for topic in rows {
        let key = topic.index.to_string();
        let mini = if series.is_empty() {
            String::new()
        } else {
            let dense = densify(by_topic.get(&key), series);
            presence(&dense, &scale)
        };
        let _ = write!(
            body,
            "<tr><td class=\"name\">{}</td><td class=\"presence\">{mini}</td>\
             <td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td>\
             <td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td>\
             <td>{}</td></tr>",
            esc(&topic.name),
            thousands(topic.messages as i64),
            thousands(topic.voices as i64),
            // A topic nobody posted in carries no average at all, and the cell
            // reads `0` rather than being left blank -- the column is numeric
            // and a hole in it looks like a rendering fault.
            topic.avg_words.map_or_else(|| "0".to_string(), number),
            thousands(topic.media as i64),
            thousands(topic.replies as i64),
            thousands(topic.reactions),
            short(&topic.top, 22)
        );
    }
    let headers = "<tr><th>Topic</th><th>Activity</th><th class=\"n\">Messages</th>\
                   <th class=\"n\">Voices</th><th class=\"n\">Words/msg</th>\
                   <th class=\"n\">Media</th><th class=\"n\">Replies</th>\
                   <th class=\"n\">Reactions</th><th>Loudest</th></tr>";
    format!(
        "{}<table><thead>{headers}</thead><tbody>{body}</tbody></table>\
         <p class=\"caption\">Same time axis as every other row on this page, \
         and one shading scale across the four. {}</p></section>",
        head("Topics", &rows.len().to_string(), "topics"),
        esc(&scale.caption("messages in a day"))
    )
}

// ---------------------------------------------------------------------------
// records
// ---------------------------------------------------------------------------

pub fn records(stats: &Stats) -> String {
    let items = &stats.superlatives;
    if items.is_empty() {
        return String::new();
    }
    let mut body = String::new();
    for item in items {
        // A superlative's value is a count for most records and a formatted
        // string for one — the largest file, which is already `1.8 MB`. The
        // count gets its group separators; the text is escaped as it stands.
        let value = match &item.value {
            Figure::Count(n) => thousands(*n),
            Figure::Text(text) => esc(text),
        };
        let unit = if item.unit.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"num\">{}</span>", esc(&item.unit))
        };
        let mut about = esc(&item.date);
        if !item.who.is_empty() {
            let _ = write!(about, " &#183; {}", esc(&item.who));
        }
        let detail = if item.text.is_empty() {
            String::new()
        } else {
            format!("<br><span>{}</span>", esc(&item.text))
        };
        let _ = write!(
            body,
            "<li><span class=\"what\">{}</span><span class=\"big\">{value}</span>\
             <span class=\"about\">{about}{unit}{detail}</span></li>",
            esc(&item.title)
        );
    }
    format!(
        "{}<ul class=\"records\">{body}</ul></section>",
        head("Records", &items.len().to_string(), "records")
    )
}

// ---------------------------------------------------------------------------
// notes
// ---------------------------------------------------------------------------

pub fn notes(stats: &Stats, events_note: &str, source: &str, stamp: &str) -> String {
    let talk = &stats.conversation;
    let folk = &stats.people;
    let mut lines: Vec<(&str, String)> = vec![
        (
            "Time",
            "Every clock face and calendar in this report reads the local \
                  timestamp the export recorded. Durations are measured on epoch \
                  seconds instead, so a gap that crosses a daylight-saving change \
                  is still the length it was."
                .to_string(),
        ),
        (
            "Identity",
            "One person is one Telegram account id, not one display name. \
                      Somebody who renamed themselves is a single row, shown under \
                      the newest name they used; <span class=\"num\">Former \
                      names</span> at the top of the page reveals the rest."
                .to_string(),
        ),
        (
            "Replies",
            format!(
                "A forum topic is itself a thread, so Telegram marks every \
             top-level message in one as a reply to the message that \
             opened the topic. Those are not answers to anybody and are \
             not counted. Replies to your own message are counted as \
             replies ({} of them) but are left out of who-answers-whom.",
                thousands(talk.self_replies)
            ),
        ),
        (
            "Reactions",
            format!(
                "Telegram names at most three reactors per message, and \
             never names anyone who reacted anonymously. Reaction \
             totals are exact; who gave them is a floor ({} of {} are attributed).",
                thousands(folk.votes_named),
                thousands(folk.votes_total)
            ),
        ),
        (
            "Bursts",
            format!(
                "A gap longer than {} minutes ends one and starts the next, \
             counted separately per topic.",
                talk.session_gap / 60
            ),
        ),
        (
            "Scale",
            "Everything with a time axis on this page runs left to right \
                   over the same span, so a mark at the same horizontal \
                   position is the same week wherever you find it. The ribbon \
                   at the top is drawn as height, because the archive is one \
                   series and height reads as quantity. The rows in the people \
                   and topic tables are shaded instead, on one scale shared by \
                   every row: at forty rows, height would draw everyone below \
                   the top few as a scatter of single pixels."
                .to_string(),
        ),
        (
            "Colour",
            "One hue, five steps, generated in OKLab and checked for \
                    even lightness steps and contrast against both backgrounds. \
                    Nothing on this page encodes a category by colour, so none \
                    of it depends on telling two hues apart."
                .to_string(),
        ),
    ];
    if !events_note.is_empty() {
        lines.insert(
            0,
            (
                "Events",
                format!(
                    "The marked events came from {}. They are somebody's \
                     reading of the archive, not a measurement of it, and each \
                     one lists the messages it rests on.",
                    esc(events_note)
                ),
            ),
        );
    }
    let rows: String = lines
        .iter()
        .map(|(title, body)| {
            format!(
                "<li><span class=\"what\">{}</span><span class=\"about\">{body}</span></li>",
                esc(title)
            )
        })
        .collect();
    format!(
        "{}<ul class=\"records\">{rows}</ul>\
         <p class=\"note\">Written by {} on {stamp} from \
         <span class=\"num\">{}</span>. No part of this report contacted Telegram; \
         it is derived entirely from the export on disk.</p></section>",
        head("Notes", "how to read this", "notes"),
        esc(source),
        esc(&stats.export.root)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_loses_its_leading_zero_and_keeps_its_month_name() {
        assert_eq!(pretty_date("2025-09-05"), "5 Sep 2025");
        assert_eq!(pretty_date("2025-12-14"), "14 Dec 2025");
        // The people table stores `first`/`last` as full timestamps.
        assert_eq!(pretty_date("2025-09-05T14:30:00"), "5 Sep 2025");
    }

    #[test]
    fn an_unparseable_date_comes_back_unchanged_rather_than_failing() {
        // A malformed timestamp in one row must not cost the whole report.
        assert_eq!(pretty_date(""), "");
        assert_eq!(pretty_date("soon"), "soon");
    }

    #[test]
    fn a_duration_climbs_units_at_the_documented_boundaries() {
        assert_eq!(duration(0.0), "0 s");
        assert_eq!(duration(89.0), "89 s");
        assert_eq!(duration(90.0), "2 min");
        assert_eq!(duration(5399.0), "90 min");
        assert_eq!(duration(5400.0), "1.5 h");
        assert_eq!(duration(129_600.0), "1.5 days");
    }

    #[test]
    fn a_short_name_is_left_whole_and_a_long_one_keeps_its_title() {
        assert_eq!(short("Ana", 22), "Ana");
        let long = short("a name far longer than the column", 22);
        assert!(long.starts_with("<span title="));
        assert!(long.ends_with("&#8230;</span>"));
    }

    #[test]
    fn an_unknown_key_labels_as_itself_rather_than_as_a_blank_cell() {
        let names = Names::new();
        assert_eq!(label_for(&names, "user123"), "user123");
        assert_eq!(label_for(&names, ""), "unknown");
    }

    #[test]
    fn argmax_takes_the_first_maximum_rather_than_the_last() {
        assert_eq!(argmax(&[1, 9, 3, 9]), 1);
        assert_eq!(argmax(&[]), 0);
    }
}
