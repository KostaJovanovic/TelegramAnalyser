//! One row per topic, on the same axis and the same shading scale.

use crate::stats::{densify, every_count, number};
use std::fmt::Write as _;
use tga_stats::Stats;

use super::*;
use crate::charts::{esc, thousands};

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
