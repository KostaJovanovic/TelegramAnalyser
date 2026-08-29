//! How the group behaves *towards each other*, and how that changes over time.
//!
//! Five figures, all of which the counting elsewhere leaves unanswerable:
//!
//! * **`pairs`** — the graph section already says who talks *at* whom. A
//!   directed edge cannot tell a conversation from a broadcast: somebody who
//!   answers everyone and is answered by nobody has the same fan-out as the
//!   person at the centre of the group. Ranking a pair by `min(there, back)`
//!   asks the other question — who talks *with* whom — and the two lists are
//!   not the same list.
//! * **`answer`** — the median reply latency is one number for an archive that
//!   behaves nothing like itself at 04:00 and at 21:00. Cut by the hour the
//!   *parent* was posted, it answers the question somebody actually has: post
//!   this now, and how long until anyone sees it.
//! * **`tenure`** — `people.rows` carries a `first` and a `last` per person and
//!   the timeline draws their ribbon, but nothing states the fact those two
//!   dates carry: whether they are still here. A row with 4,000 messages whose
//!   last one was eight months ago is not a description of the group today.
//! * **`retention`** — `churn` counts arrivals and departures Telegram
//!   *announced*, which misses everyone who simply stopped typing. Month over
//!   month, of the people who spoke last month, how many spoke again.
//! * **`depth`** — reply chains. `conversation` measures the first hop; this
//!   measures how far a thread actually runs before it dies.
//!
//! Two inherited rules are load-bearing here and are not re-derived:
//!
//! *Latency is measured on epoch seconds*, never the wall clock, and the same
//! [`LATENCY_CAP`] applies — a reply three days later is somebody returning to
//! a thread, not a response time.
//!
//! *A reply to the message that opened a forum topic is not a reply.* The
//! reader drops those once, so the chains below are threads rather than the
//! whole topic hanging off its own first message.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, Timelike};
use tga_read::{Export, Msg};
use tga_stats::{
    Answer, Count, Deepest, Depth, Dynamics, HourPick, Pair, Pairs, Retention, Tenure, TenureRow,
};

use crate::conversation::{median, LATENCY_CAP};
use crate::identity::People;
use crate::util::{round1, Counter};

/// How many correspondent pairs get a row. Past this the list stops ranking
/// anything — the tail is every pair that exchanged two messages once.
const PAIRS_SHOWN: usize = 20;

/// Replies needed in an hour before its median is quoted as the group's
/// behaviour. Below it, one slow afternoon *is* the median.
const ANSWER_MIN: i64 = 20;

/// Days since somebody's last message, and what to call the silence.
///
/// Stated as constants and reported in the output rather than hidden in a
/// comparison, so a reader who thinks a month is too generous can say so.
const DORMANT_ACTIVE: i64 = 30;
const DORMANT_FADING: i64 = 90;

/// Chain lengths are reported individually up to here and pooled above it.
const DEPTH_CAP: i64 = 8;

/// A chain longer than this is not a thread.
///
/// It exists as a stop rather than as a figure: `reply_to` comes out of a file,
/// and a file that points a message at its own descendant walks forever. The
/// cycle guard below catches the common shape; this catches the rest.
const MAX_CHAIN: usize = 10_000;

pub fn compute(export: &Export, people: &People) -> Dynamics {
    let msgs: Vec<&Msg> = export.said().collect();
    if msgs.is_empty() {
        return Dynamics {
            empty: true,
            ..Default::default()
        };
    }
    let index = export.by_id();

    Dynamics {
        empty: false,
        pairs: pairs(&msgs, export, &index, people),
        answer: answer(&msgs, export, &index),
        tenure: tenure(&msgs, people),
        retention: retention(&msgs, people),
        depth: depth(&msgs, export, &index),
    }
}

// ---------------------------------------------------------------------------
// who talks with whom
// ---------------------------------------------------------------------------

/// Reply edges, collapsed onto unordered pairs.
///
/// The ranking key is `min(there, back)` rather than the total, and that choice
/// is the whole figure. A pair where one person sent 300 replies and got 2 back
/// has a large total and is not a correspondence; ranked on the smaller
/// direction it falls where it belongs, and `balance` says how lopsided it was.
fn pairs(msgs: &[&Msg], export: &Export, index: &HashMap<i64, usize>, people: &People) -> Pairs {
    // Insertion-ordered, so the pair set below is built in a deterministic
    // order however the hashes fall. Same reasoning as `util::Counter`'s.
    let mut directed: Counter<(String, String)> = Counter::new();
    for msg in msgs {
        let Some(parent) = parent_of(msg, export, index) else {
            continue;
        };
        let (source, target) = (people.key_of(msg), people.key_of(parent));
        // A reply to yourself is dropped here for the reason `conversation`
        // drops it from the graph: everyone talks to themselves, and counting
        // it makes the loudest person their own closest correspondent.
        if source.is_empty() || target.is_empty() || source == target {
            continue;
        }
        directed.bump((source, target));
    }

    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut ranked: Vec<(i64, i64, String, String)> = Vec::new();
    let mut mutual = 0i64;
    let mut one_way = 0i64;
    for (from, to) in directed.keys() {
        let pair = if from <= to {
            (from.clone(), to.clone())
        } else {
            (to.clone(), from.clone())
        };
        if !seen.insert(pair.clone()) {
            continue;
        }
        let there = directed.get(&(pair.0.clone(), pair.1.clone()));
        let back = directed.get(&(pair.1.clone(), pair.0.clone()));
        if there.min(back) > 0 {
            mutual += 1;
        } else {
            one_way += 1;
        }
        ranked.push((there.min(back), there + back, pair.0, pair.1));
    }
    // Total, down to the keys: two pairs with the same two counts must not
    // change places between runs, and the keys are the only thing left that
    // tells them apart.
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
    });

    let rows: Vec<Pair> = ranked
        .iter()
        .take(PAIRS_SHOWN)
        .map(|(both, total, a, b)| {
            let a_to_b = directed.get(&(a.clone(), b.clone()));
            let b_to_a = directed.get(&(b.clone(), a.clone()));
            let most = a_to_b.max(b_to_a);
            Pair {
                a_name: people.name_of(a),
                b_name: people.name_of(b),
                a: a.clone(),
                b: b.clone(),
                a_to_b,
                b_to_a,
                both: *both,
                replies: *total,
                // 1.0 is an even exchange, 0.0 is one person talking. Emitted
                // raw rather than rounded, like every other share in the dump.
                balance: if most > 0 {
                    *both as f64 / most as f64
                } else {
                    0.0
                },
            }
        })
        .collect();

    Pairs {
        shown: rows.len(),
        rows,
        mutual,
        one_way,
        directed: directed.len(),
    }
}

// ---------------------------------------------------------------------------
// when the group is awake
// ---------------------------------------------------------------------------

/// Reply latency, cut by the hour the message being answered was posted.
///
/// Keyed on the **parent's** hour, not the reply's. "How long until somebody
/// answers me" is a question about when you posted; keying it on when the
/// answer arrived measures the answerer's habits instead, which is a different
/// question and the one `conversation::fastest` already covers.
fn answer(msgs: &[&Msg], export: &Export, index: &HashMap<i64, usize>) -> Answer {
    let mut buckets: Vec<Vec<i64>> = vec![Vec::new(); 24];
    for msg in msgs {
        let Some(parent) = parent_of(msg, export, index) else {
            continue;
        };
        let gap = msg.unix - parent.unix;
        if (0..=LATENCY_CAP).contains(&gap) {
            buckets[parent.when.hour() as usize].push(gap);
        }
    }

    let mut counts = [0i64; 24];
    let mut medians = [0i64; 24];
    for (hour, values) in buckets.iter().enumerate() {
        counts[hour] = values.len() as i64;
        medians[hour] = if values.is_empty() {
            0
        } else {
            median(values).trunc() as i64
        };
    }

    // Only hours with enough replies to mean something get to be the best or
    // the worst. Without the floor both are won by 04:00, where three replies
    // landed and one of them was instant.
    let solid: Vec<usize> = (0..24).filter(|h| counts[*h] >= ANSWER_MIN).collect();
    let pick = |wanted: &dyn Fn(i64, i64) -> bool| -> Option<HourPick> {
        let mut best: Option<usize> = None;
        for hour in &solid {
            match best {
                Some(current) if !wanted(medians[*hour], medians[current]) => {}
                _ => best = Some(*hour),
            }
        }
        best.map(|hour| HourPick {
            hour,
            median: medians[hour],
            replies: counts[hour],
        })
    };

    Answer {
        counted: counts.iter().sum::<i64>(),
        counts,
        medians,
        minimum: ANSWER_MIN,
        cap: LATENCY_CAP,
        fastest_hour: pick(&|candidate, current| candidate < current),
        slowest_hour: pick(&|candidate, current| candidate > current),
    }
}

// ---------------------------------------------------------------------------
// who is still here
// ---------------------------------------------------------------------------

struct Span {
    first: NaiveDate,
    last: NaiveDate,
    days: HashSet<NaiveDate>,
    messages: i64,
    name: String,
}

/// Where each person sits between their first message and the end of the
/// archive.
///
/// `dormant` is measured against the **archive's** last day rather than today,
/// because an export is a fixed document: reading the same file a year later
/// must not silently reclassify everyone in it as gone.
fn tenure(msgs: &[&Msg], people: &People) -> Tenure {
    let archive_last = msgs[msgs.len() - 1].when.date();

    let mut order: Vec<String> = Vec::new();
    let mut spans: HashMap<String, Span> = HashMap::new();
    for msg in msgs {
        let key = people.key_of(msg);
        if key.is_empty() {
            continue;
        }
        let day = msg.when.date();
        let span = spans.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Span {
                first: day,
                last: day,
                days: HashSet::new(),
                messages: 0,
                name: people.name_of(&key),
            }
        });
        span.first = span.first.min(day);
        span.last = span.last.max(day);
        span.days.insert(day);
        span.messages += 1;
    }

    let mut rows: Vec<TenureRow> = Vec::new();
    let (mut active, mut fading, mut gone) = (0i64, 0i64, 0i64);
    let mut ranked: Vec<&String> = order.iter().collect();
    // Same order as `people.rows`, so the two tables can be read side by side.
    ranked.sort_by(|a, b| {
        spans[*b].messages.cmp(&spans[*a].messages).then_with(|| {
            spans[*a]
                .name
                .to_lowercase()
                .cmp(&spans[*b].name.to_lowercase())
        })
    });
    for key in ranked {
        let span = &spans[key];
        let dormant = (archive_last - span.last).num_days();
        let status = if dormant <= DORMANT_ACTIVE {
            active += 1;
            "active"
        } else if dormant <= DORMANT_FADING {
            fading += 1;
            "fading"
        } else {
            gone += 1;
            "gone"
        };
        let width = (span.last - span.first).num_days() + 1;
        rows.push(TenureRow {
            key: key.clone(),
            name: span.name.clone(),
            messages: span.messages,
            first: span.first.to_string(),
            last: span.last.to_string(),
            span_days: width,
            active_days: span.days.len(),
            // What share of the days they were around on did they actually
            // speak. A regular with a short tenure scores above somebody who
            // has been here for years and posts twice a season.
            density: if width > 0 {
                span.days.len() as f64 / width as f64
            } else {
                0.0
            },
            dormant_days: dormant,
            status: status.to_string(),
        });
    }

    Tenure {
        rows,
        as_of: archive_last.to_string(),
        active,
        fading,
        gone,
        active_within: DORMANT_ACTIVE,
        fading_within: DORMANT_FADING,
    }
}

// ---------------------------------------------------------------------------
// month over month
// ---------------------------------------------------------------------------

/// Of the people who spoke last month, how many spoke again.
///
/// `churn` reports the arrivals and departures Telegram *announced*, which is
/// the only kind it can see. Nobody is announced for going quiet, and going
/// quiet is how a group actually ends — so this counts the four states that
/// membership list cannot: still here, back after a gap, new, and lost.
fn retention(msgs: &[&Msg], people: &People) -> Retention {
    let month_of = |day: NaiveDate| format!("{:04}-{:02}", day.year(), day.month());

    let mut spoke: HashMap<String, HashSet<String>> = HashMap::new();
    for msg in msgs {
        let key = people.key_of(msg);
        if key.is_empty() {
            continue;
        }
        spoke
            .entry(month_of(msg.when.date()))
            .or_default()
            .insert(key);
    }

    // The axis is every month between the first and the last, silent ones
    // included. Taking only the months that appear closes a six-month gap into
    // a single step and reports the return after it as ordinary retention.
    let (first, last) = (msgs[0].when.date(), msgs[msgs.len() - 1].when.date());
    let mut months: Vec<String> = Vec::new();
    let (mut year, mut month) = (first.year(), first.month());
    while (year, month) <= (last.year(), last.month()) {
        months.push(format!("{year:04}-{month:02}"));
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }

    let empty: HashSet<String> = HashSet::new();
    let mut before: HashSet<String> = HashSet::new();
    let mut previous: &HashSet<String> = &empty;
    let (mut act, mut fresh, mut back, mut lost) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut kept: Vec<f64> = Vec::new();

    for name in &months {
        let now = spoke.get(name).unwrap_or(&empty);
        act.push(now.len() as i64);
        fresh.push(now.iter().filter(|k| !before.contains(*k)).count() as i64);
        back.push(now.intersection(previous).count() as i64);
        lost.push(previous.difference(now).count() as i64);
        if !previous.is_empty() {
            kept.push(now.intersection(previous).count() as f64 / previous.len() as f64);
        }
        before.extend(now.iter().cloned());
        previous = now;
    }

    Retention {
        months,
        active: act,
        new: fresh,
        returning: back,
        lost,
        people: before.len(),
        // The mean of the monthly rates, not the ratio of the totals: every
        // month gets one vote, so a single enormous month cannot speak for the
        // years around it. Emitted unrounded, like `share` and `reply_share` —
        // rounding to a percent here and dividing back by 100 puts float noise
        // in the dump and still hands the report the same figure.
        kept_mean: if kept.is_empty() {
            0.0
        } else {
            kept.iter().sum::<f64>() / kept.len() as f64
        },
        months_counted: kept.len(),
    }
}

// ---------------------------------------------------------------------------
// how far a thread runs
// ---------------------------------------------------------------------------

/// Reply-chain length: how many messages deep a thread got.
///
/// Depth 1 is every message nobody was answering, so it is most of the archive
/// and says nothing; the distribution below starts at 2, where a chain begins.
///
/// The walk is iterative rather than recursive on purpose. A chain is bounded
/// by nothing but the file, and 300,000 stack frames is a crash rather than a
/// wrong number — which is the worse failure of the two.
fn depth(msgs: &[&Msg], export: &Export, index: &HashMap<i64, usize>) -> Depth {
    let mut known: HashMap<i64, i64> = HashMap::new();
    for msg in &export.msgs {
        if known.contains_key(&msg.id) {
            continue;
        }
        let mut chain: Vec<i64> = Vec::new();
        let mut on_stack: HashSet<i64> = HashSet::new();
        let mut current = msg;
        let base = loop {
            if let Some(&settled) = known.get(&current.id) {
                break settled;
            }
            // A file can point a message at its own descendant. Neither of
            // these guards produces a figure; they stop a walk that would not
            // return, and the chain built so far is still counted from 0.
            if !on_stack.insert(current.id) || chain.len() >= MAX_CHAIN {
                break 0;
            }
            chain.push(current.id);
            match current.reply_to.and_then(|parent| index.get(&parent)) {
                Some(&at) => current = &export.msgs[at],
                None => break 0,
            }
        };
        for (step, id) in chain.iter().rev().enumerate() {
            known.insert(*id, base + step as i64 + 1);
        }
    }

    let mut counts: Counter<i64> = Counter::new();
    let mut lengths: Vec<i64> = Vec::new();
    let mut deepest: Option<&Msg> = None;
    for msg in msgs {
        let at = known.get(&msg.id).copied().unwrap_or(1);
        if at < 2 {
            continue;
        }
        lengths.push(at);
        counts.bump(at.min(DEPTH_CAP + 1));
        // Ties go to the lower id, which is the rule `extras::superlatives`
        // uses for every other record on the page.
        deepest = match deepest {
            Some(best) if known.get(&best.id).copied().unwrap_or(1) >= at => Some(best),
            _ => Some(msg),
        };
    }

    let mut buckets: Vec<Count> = Vec::new();
    for at in 2..=DEPTH_CAP + 1 {
        let label = if at > DEPTH_CAP {
            format!("{DEPTH_CAP}+")
        } else {
            at.to_string()
        };
        buckets.push(Count::new(label, counts.get(&at)));
    }

    let longest = deepest.map(|msg| Deepest {
        id: msg.id,
        topic: msg.topic,
        date: crate::util::stamp_minutes(&msg.when),
        messages: known.get(&msg.id).copied().unwrap_or(1),
    });

    Depth {
        buckets,
        cap: DEPTH_CAP,
        chained: lengths.len(),
        max: lengths.iter().copied().max().unwrap_or(0),
        median: if lengths.is_empty() {
            0
        } else {
            median(&lengths).trunc() as i64
        },
        mean: if lengths.is_empty() {
            0.0
        } else {
            round1(lengths.iter().sum::<i64>() as f64 / lengths.len() as f64)
        },
        longest,
    }
}

// ---------------------------------------------------------------------------

/// The message a reply is answering, if the export contains it.
///
/// An orphan — a reply whose parent is outside the export — is skipped rather
/// than counted as a root. `conversation` reports how many there are; nothing
/// here can say anything true about one.
fn parent_of<'a>(msg: &Msg, export: &'a Export, index: &HashMap<i64, usize>) -> Option<&'a Msg> {
    let at = *index.get(&msg.reply_to?)?;
    Some(&export.msgs[at])
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::NaiveDate;
    use tga_read::{Msg, Topic};

    fn at(day: u32, hour: u32, minute: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 1, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn msg(id: i64, who: &str, when: chrono::NaiveDateTime, reply_to: Option<i64>) -> Msg {
        Msg {
            id,
            topic: 0,
            when,
            unix: when.and_utc().timestamp(),
            service: false,
            action: String::new(),
            sender: who.to_string(),
            name: who.to_string(),
            text: "hello".into(),
            chars: 5,
            words: 1,
            reply_to,
            forward_from: String::new(),
            forward_id: String::new(),
            media: String::new(),
            file_size: 0,
            media_saved: false,
            sticker_emoji: String::new(),
            emoji: Vec::new(),
            domains: Vec::new(),
            mentions: Vec::new(),
            hashtags: Vec::new(),
            reactions: Vec::new(),
            edited: false,
            members: Vec::new(),
            grouped: None,
        }
    }

    fn export(msgs: Vec<Msg>) -> Export {
        Export {
            name: "test".into(),
            topics: vec![Topic {
                index: 0,
                name: "General".into(),
                folder: Default::default(),
                chat_id: None,
                chat_type: String::new(),
                created: None,
                messages: msgs.len(),
                root: None,
            }],
            msgs,
            ..Default::default()
        }
    }

    /// Two people answering each other, and one person answering into the void.
    fn conversation() -> Export {
        export(vec![
            msg(1, "user1", at(1, 10, 0), None),
            msg(2, "user2", at(1, 10, 5), Some(1)),
            msg(3, "user1", at(1, 10, 9), Some(2)),
            msg(4, "user2", at(1, 10, 30), Some(3)),
            msg(5, "user3", at(1, 11, 0), Some(1)),
            msg(6, "user3", at(1, 11, 4), Some(1)),
        ])
    }

    #[test]
    fn a_pair_is_ranked_on_the_direction_that_answered_least() {
        let export = conversation();
        let people = People::new(&export);
        let out = compute(&export, &people);
        let rows = &out.pairs.rows;

        // user1<->user2 exchanged three replies, two one way and one the
        // other; user3 sent two and received none. The lopsided pair must not
        // outrank the correspondence just for having a similar total.
        assert_eq!(rows[0].both, 1);
        assert_eq!(rows[0].replies, 3);
        assert_eq!(rows[1].both, 0);
        assert_eq!(out.pairs.mutual, 1);
        assert_eq!(out.pairs.one_way, 1);
    }

    #[test]
    fn a_reply_to_yourself_is_not_a_correspondence() {
        let export = export(vec![
            msg(1, "user1", at(1, 10, 0), None),
            msg(2, "user1", at(1, 10, 1), Some(1)),
        ]);
        let people = People::new(&export);
        let out = compute(&export, &people);
        assert!(out.pairs.rows.is_empty());
        assert_eq!(out.pairs.directed, 0);
    }

    #[test]
    fn latency_is_bucketed_by_the_hour_the_parent_was_posted() {
        let export = conversation();
        let people = People::new(&export);
        let out = compute(&export, &people);
        let counts = out.answer.counts;
        // Four replies answer messages posted at 10:00, one answers 10:09, and
        // none answers anything posted at 11:00 — the two 11:00 messages are
        // replies themselves, and a reply's own hour is not what this counts.
        assert_eq!(counts[10], 5);
        assert_eq!(counts[11], 0);
        // Well under ANSWER_MIN, so neither superlative is claimed.
        assert!(out.answer.fastest_hour.is_none());
    }

    #[test]
    fn dormancy_is_measured_against_the_archive_rather_than_today() {
        let export = export(vec![
            msg(1, "user1", at(1, 10, 0), None),
            msg(2, "user2", at(1, 10, 5), None),
            msg(3, "user1", at(31, 10, 0), None),
        ]);
        let people = People::new(&export);
        let out = compute(&export, &people);
        let rows = &out.tenure.rows;
        assert_eq!(out.tenure.as_of, "2025-01-31");
        assert_eq!(rows[0].dormant_days, 0);
        assert_eq!(rows[0].span_days, 31);
        assert_eq!(rows[0].active_days, 2);
        assert_eq!(rows[1].dormant_days, 30);
        // 30 days is still inside DORMANT_ACTIVE, and the boundary is the
        // thing worth pinning: an off-by-one here silently reclassifies
        // everyone who posts monthly.
        assert_eq!(rows[1].status, "active");
        assert_eq!(out.tenure.active, 2);
    }

    #[test]
    fn a_silent_month_stays_on_the_axis() {
        let export = export(vec![
            msg(1, "user1", at(1, 10, 0), None),
            msg(2, "user2", at(2, 10, 0), None),
            // March, after a silent February.
            msg(
                3,
                "user1",
                NaiveDate::from_ymd_opt(2025, 3, 4)
                    .unwrap()
                    .and_hms_opt(9, 0, 0)
                    .unwrap(),
                None,
            ),
        ]);
        let people = People::new(&export);
        let out = compute(&export, &people);
        let ret = &out.retention;
        assert_eq!(ret.months, ["2025-01", "2025-02", "2025-03"]);
        assert_eq!(ret.active, [2, 0, 1]);
        assert_eq!(ret.new, [2, 0, 0]);
        // Nobody returned in March, because February is what March is measured
        // against and February was empty. Collapsing the gap would have called
        // this a return.
        assert_eq!(ret.returning, [0, 0, 0]);
        assert_eq!(ret.lost, [0, 2, 0]);
    }

    #[test]
    fn depth_counts_the_chain_not_the_hop() {
        let export = conversation();
        let people = People::new(&export);
        let out = compute(&export, &people);
        let depth = &out.depth;
        // 1 <- 2 <- 3 <- 4 is a chain of four; 5 and 6 both sit at two.
        assert_eq!(depth.max, 4);
        assert_eq!(depth.chained, 5);
        assert_eq!(depth.longest.as_ref().expect("a deepest chain").id, 4);
        assert_eq!(depth.buckets[0], Count::new("2", 3));
        assert_eq!(depth.buckets[1], Count::new("3", 1));
        assert_eq!(depth.buckets[2], Count::new("4", 1));
    }

    #[test]
    fn a_reply_cycle_does_not_hang_the_walk() {
        // Nothing Telegram writes looks like this. A hand-edited events file or
        // a truncated export can, and a metric that never returns takes the
        // whole report with it.
        let export = export(vec![
            msg(1, "user1", at(1, 10, 0), Some(2)),
            msg(2, "user2", at(1, 10, 1), Some(1)),
        ]);
        let people = People::new(&export);
        let out = compute(&export, &people);
        assert!(out.depth.max >= 1);
    }

    #[test]
    fn an_export_with_nothing_said_says_so() {
        let export = export(vec![]);
        let people = People::new(&export);
        let out = compute(&export, &people);
        assert!(out.empty);
        assert!(out.pairs.rows.is_empty());
        assert!(out.tenure.rows.is_empty());
    }
}
