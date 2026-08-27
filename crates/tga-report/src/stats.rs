//! Reading the metrics structure.
//!
//! The report renders from a `serde_json::Value` of exactly the shape
//! `tga_metrics::analyse` returns and `--stats` dumps, the way `report.py`
//! renders from a plain dict. That is what keeps this crate off `tga-read` and
//! `tga-metrics`, and it is what lets a recorded fixture replay through the
//! writer with no export on disk.
//!
//! **Every accessor here has a total answer.** A missing branch renders as an
//! empty section, never as a panic: the writer is handed a file that something
//! else produced, and half a report is worth more than a stack trace.

use serde_json::Value;

use crate::charts::Day;

pub fn s<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub fn i(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

pub fn f(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn arr<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// A number the way Python's `str()` prints it.
///
/// `avg_words` and `mean_words` are `round(x, 1)` on the Python side, so they
/// are floats and print as `12.0` rather than `12`. serde_json writes the
/// shortest float that round-trips, which is the same rule Python's `repr`
/// uses, so handing the number's own serialisation through is exact.
pub fn number(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(text)) => text.clone(),
        _ => "0".to_string(),
    }
}

/// An `[i64, i64, ...]` branch, as `per_hour` and `per_weekday` are.
pub fn ints(value: &Value, key: &str) -> Vec<i64> {
    arr(value, key)
        .iter()
        .map(|v| v.as_i64().unwrap_or(0))
        .collect()
}

/// A `[[i64, ...], ...]` branch, as `hour_weekday` is.
pub fn grid(value: &Value, key: &str) -> Vec<Vec<i64>> {
    arr(value, key)
        .iter()
        .map(|row| {
            row.as_array()
                .map(|cells| cells.iter().map(|c| c.as_i64().unwrap_or(0)).collect())
                .unwrap_or_default()
        })
        .collect()
}

/// A `[[label, count], ...]` branch — `per_day`, `per_month`, `emoji`,
/// `domains` and the rest of the counter dumps all share this shape.
pub fn pairs(value: &Value, key: &str) -> Vec<Day> {
    arr(value, key)
        .iter()
        .filter_map(|row| {
            let row = row.as_array()?;
            Some((
                match row.first()? {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                },
                row.get(1).and_then(Value::as_i64).unwrap_or(0),
            ))
        })
        .collect()
}

/// A sparse `{day: count}` map, densified against the shared axis.
///
/// `per_day_by_person` and `per_day_by_topic` are stored sparse — dense they
/// would be one entry per person per day of the archive, which for a large
/// group is millions of zeroes nobody reads. The report draws each as a row on
/// the same axis as the whole-archive ribbon, so the zeroes are put back here,
/// at draw time, against that axis.
pub fn densify(sparse: Option<&Value>, axis: &[Day]) -> Vec<Day> {
    let counts = sparse.and_then(Value::as_object);
    axis.iter()
        .map(|(day, _)| {
            let n = counts
                .and_then(|map| map.get(day))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            (day.clone(), n)
        })
        .collect()
}

/// Every value in a sparse-by-key branch, for building one shared scale.
pub fn every_count(branch: &Value) -> Vec<i64> {
    branch
        .as_object()
        .map(|outer| {
            outer
                .values()
                .filter_map(Value::as_object)
                .flat_map(|inner| inner.values().filter_map(Value::as_i64))
                .collect()
        })
        .unwrap_or_default()
}

/// Truncating, like Desktop's own writer — 1,940,744 B is `1.8 MB`.
///
/// Mirrors `tga_metrics::extras::human_bytes`; duplicated rather than imported
/// because this crate must not depend on `tga-metrics` (see `Cargo.toml`). Ten
/// lines is a cheaper price than the layering rule, and the test below pins the
/// two to the same answers.
pub fn human_bytes(size: i64) -> String {
    let step = 1024.0f64;
    let mut value = size as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if value < step || unit == "GB" {
            if unit == "B" {
                return format!("{} B", value.trunc() as i64);
            }
            return format!("{:.1} {}", (value * 10.0).trunc() / 10.0, unit);
        }
        value /= step;
    }
    format!("{value:.1} GB")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn human_bytes_truncates_rather_than_rounds() {
        // The number in the docs, and the reason it is not `{:.1}` alone:
        // 1.85 MB rounds up to 1.9 and Desktop writes 1.8.
        assert_eq!(human_bytes(1_940_744), "1.8 MB");
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(1024 * 1024 * 1024 * 3), "3.0 GB");
    }

    #[test]
    fn a_missing_branch_reads_as_empty_rather_than_panicking() {
        let empty = json!({});
        assert_eq!(s(&empty, "name"), "");
        assert_eq!(i(&empty, "messages"), 0);
        assert!(arr(&empty, "rows").is_empty());
        assert!(pairs(&empty, "per_day").is_empty());
        assert_eq!(number(&empty, "avg_words"), "0");
    }

    #[test]
    fn a_rounded_float_keeps_its_trailing_zero_the_way_python_prints_it() {
        let row = json!({ "avg_words": 12.0, "other": 12.5 });
        assert_eq!(number(&row, "avg_words"), "12.0");
        assert_eq!(number(&row, "other"), "12.5");
    }

    #[test]
    fn a_sparse_person_is_densified_against_the_shared_axis() {
        let axis: Vec<Day> = vec![
            ("2025-01-01".into(), 5),
            ("2025-01-02".into(), 0),
            ("2025-01-03".into(), 2),
        ];
        let sparse = json!({ "2025-01-03": 2 });
        let dense = densify(Some(&sparse), &axis);
        assert_eq!(
            dense,
            vec![
                ("2025-01-01".to_string(), 0),
                ("2025-01-02".to_string(), 0),
                ("2025-01-03".to_string(), 2),
            ]
        );
    }

    #[test]
    fn a_person_absent_from_the_branch_densifies_to_silence_not_to_nothing() {
        // A row of no cells and a row of zero cells are different pictures: the
        // second says "here is this person's row, and it is empty".
        let axis: Vec<Day> = vec![("2025-01-01".into(), 5)];
        assert_eq!(densify(None, &axis), vec![("2025-01-01".to_string(), 0)]);
    }
}
