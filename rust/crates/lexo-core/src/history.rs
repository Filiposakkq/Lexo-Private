//! Port of `reference/python/core/lexo/history.py`: `history.csv`, the progress log, and the completion estimates and weekly goals computed from it.
//!
//! history.csv columns: date;translated;total;percent;proofread;proofread_percent. Existing rows are never edited except the newest one of the running session (a session
//! keeps refreshing "its" row while the translator works). The functions that need "today" take it as a parameter.
use crate::clock::Clock;
use crate::fsutil::{read_rows, write_csv, WriteFailed};
use crate::messages::{msg, msgn, Message};
use crate::pyfmt::{float, int_of_float, round_digits, round_half_even};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const HEADER: [&str; 6] = ["date", "translated", "total", "percent", "proofread", "proofread_percent"];
pub const STAMP_FORMAT: &str = "%Y-%m-%d %H:%M";

/// Estimate requirements, deliberately conservative: the history must cover at least a week, on at least three different days.
pub const MIN_SPAN_DAYS: i64 = 7;
pub const MIN_DAYS_WITH_DATA: usize = 3;
/// The pace is measured over the most recent four weeks.
pub const WINDOW_DAYS: i64 = 28;
/// Further than this is not a forecast, it is noise.
pub const HORIZON_DAYS: f64 = 365.0 * 5.0;

/// (time, translated, total, proofread)
pub type Point = (NaiveDateTime, i64, i64, i64);

fn num(s: &str) -> Option<f64> {
    float(&s.replace(',', "."))
}

/// The numbers of one log row: when, translated, total, percent translated, proofread, percent proofread.
pub struct Progress {
    pub stamp: String,
    pub t_done: i64,
    pub total: i64,
    pub tpct: f64,
    pub c_done: i64,
    pub cpct: f64,
}

pub struct History {
    pub path: PathBuf,
    backup_dir: Option<PathBuf>,
    /// Did this session already add or adopt a row?
    session_row: bool,
}

impl History {
    pub fn new(path: &Path, backup_dir: Option<&Path>) -> History {
        History { path: path.to_path_buf(), backup_dir: backup_dir.map(Path::to_path_buf), session_row: false }
    }

    /// Raw rows without the header, padded to the full width.
    pub fn rows(&self) -> Vec<Vec<String>> {
        read_rows(&self.path)
            .unwrap_or_default()
            .into_iter()
            .skip(1)
            .map(|mut r| {
                while r.len() < HEADER.len() {
                    r.push(String::new());
                }
                r
            })
            .collect()
    }

    /// Every parsable row, oldest first.
    pub fn points(&self) -> Vec<Point> {
        self.rows()
            .iter()
            .filter_map(|r| {
                let ts = NaiveDateTime::parse_from_str(&r[0], STAMP_FORMAT).ok()?;
                let proof = if r[4].is_empty() { 0 } else { int_of_float(&r[4].replace(',', "."))? };
                Some((ts, int_of_float(&r[1].replace(',', "."))?, int_of_float(&r[2].replace(',', "."))?, proof))
            })
            .collect()
    }

    /// (time, translation %, proofreading %): what the website chart plots.
    pub fn chart_points(&self) -> Vec<(NaiveDateTime, f64, f64)> {
        self.rows()
            .iter()
            .filter_map(|r| {
                let ts = NaiveDateTime::parse_from_str(&r[0], STAMP_FORMAT).ok()?;
                let proof = if r[5].is_empty() { 0.0 } else { num(&r[5])? };
                Some((ts, num(&r[3])?, proof))
            })
            .collect()
    }

    /// Log the current state. Appends one row per session (and only when something changed), afterwards the session's own row is updated. True when the file was written.
    pub fn record(&mut self, p: &Progress, clock: &dyn Clock) -> Result<bool, WriteFailed> {
        let mut body = self.rows();
        let entry: Vec<String> =
            vec![p.stamp.clone(), p.t_done.to_string(), p.total.to_string(), format!("{:.1}", p.tpct), p.c_done.to_string(), format!("{:.1}", p.cpct)];
        if let Some(last) = body.last() {
            if last[1..3] == entry[1..3] && last[4] == entry[4] {
                return Ok(false); // nothing changed since the last row
            }
        }
        if self.session_row && !body.is_empty() {
            *body.last_mut().unwrap() = entry;
        } else {
            body.push(entry);
            self.session_row = true;
        }
        let mut rows: Vec<Vec<String>> = vec![HEADER.iter().map(|s| s.to_string()).collect()];
        rows.extend(body);
        write_csv(&self.path, &rows, self.backup_dir.as_deref(), clock)?;
        Ok(true)
    }
}

// ---------------------------------------------------------------- estimates

/// Last record of every calendar day: (date, translated, total), oldest first.
pub fn daily_series(points: &[Point]) -> Vec<(NaiveDate, i64, i64)> {
    let mut by_day: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    for (ts, done, total, _corr) in points {
        by_day.insert(ts.date(), (*done, *total));
    }
    by_day.into_iter().map(|(d, v)| (d, v.0, v.1)).collect()
}

/// Translated count on `day` (step function: last known value on or before it).
fn value_at(series: &[(NaiveDate, i64, i64)], day: NaiveDate) -> Option<i64> {
    series.iter().take_while(|(d, _, _)| *d <= day).last().map(|(_, done, _)| *done)
}

#[derive(Debug)]
pub struct Estimate {
    /// `ok`, `complete`, `insufficient_data`, `no_progress` or `too_far`
    pub status: &'static str,
    pub remaining: i64,
    pub pace_per_day: Option<f64>,
    pub eta: Option<NaiveDate>,
    pub eta_early: Option<NaiveDate>,
    pub eta_late: Option<NaiveDate>,
    pub confidence: Option<&'static str>,
    pub basis_days: i64,
    pub days_with_data: usize,
    pub reason: Option<Message>,
}

impl Estimate {
    pub fn to_json(&self) -> Value {
        let date = |d: &Option<NaiveDate>| d.map_or(Value::Null, |d| json!(d.format("%Y-%m-%d").to_string()));
        json!({
            "status": self.status, "remaining": self.remaining, "pace_per_day": self.pace_per_day, "eta": date(&self.eta), "eta_early": date(&self.eta_early),
            "eta_late": date(&self.eta_late), "confidence": self.confidence, "basis_days": self.basis_days, "days_with_data": self.days_with_data,
            "reason": self.reason.as_ref().map_or(json!(""), Message::wire),
        })
    }
}

/// Projected completion of the *remaining* work from the recent pace. The status is `ok` only when the history supports a forecast; otherwise it names the reason and
/// no date is given. Nothing is extrapolated from a handful of hours of data.
pub fn estimate(points: &[Point], done_now: i64, total_now: i64, today: NaiveDate) -> Estimate {
    let remaining = (total_now - done_now).max(0);
    let mut out = Estimate {
        status: "insufficient_data",
        remaining,
        pace_per_day: None,
        eta: None,
        eta_early: None,
        eta_late: None,
        confidence: None,
        basis_days: 0,
        days_with_data: 0,
        reason: None,
    };
    if remaining == 0 {
        out.status = "complete";
        out.reason = Some(msg("Everything is translated.", &[]));
        return out;
    }
    let series = daily_series(points);
    if series.is_empty() {
        out.reason = Some(msg("The history is empty – the estimate cannot be calculated yet.", &[]));
        return out;
    }
    let window_start = today - Duration::days(WINDOW_DAYS);
    let in_window: Vec<_> = series.iter().filter(|s| s.0 >= window_start).copied().collect();
    // baseline: last known value before the window, else the oldest record we have
    let before: Vec<_> = series.iter().filter(|s| s.0 < window_start).copied().collect();
    let base = before.last().copied().unwrap_or(series[0]);
    let mut used: Vec<_> = if in_window.contains(&base) { Vec::new() } else { vec![base] };
    used.extend(in_window.iter().copied());
    let days_with_data = used.iter().map(|s| s.0).collect::<std::collections::BTreeSet<_>>().len();
    let span = (today - base.0).num_days();
    out.basis_days = span;
    out.days_with_data = days_with_data;
    if span < MIN_SPAN_DAYS || days_with_data < MIN_DAYS_WITH_DATA {
        let covers = if span < 1 { msg("only one day", &[]) } else { msgn(span, "{n} day", "{n} days", &[]) };
        out.reason = Some(msg(
            "Not enough data yet: the history covers {covers}, the records are from {days} days (at least {min_span} days and {min_days} days with a record are needed).",
            &[("covers", covers.wire()), ("days", json!(days_with_data)), ("min_span", json!(MIN_SPAN_DAYS)), ("min_days", json!(MIN_DAYS_WITH_DATA))],
        ));
        return out;
    }
    let net = done_now - base.1;
    if net <= 0 {
        out.status = "no_progress";
        out.reason = Some(msg("No translated lines were added in recent weeks – the date cannot be estimated.", &[]));
        return out;
    }
    let pace = net as f64 / span.max(1) as f64;
    out.pace_per_day = Some(round_digits(pace, 1));
    let eta_for = |rate: f64| {
        let days = remaining as f64 / rate;
        (days <= HORIZON_DAYS).then(|| today + Duration::days(round_half_even(days) as i64))
    };
    out.eta = eta_for(pace);
    // spread: pace of each full week inside the window
    let mut weekly: Vec<f64> = Vec::new();
    let mut d = base.0;
    while d + Duration::days(7) <= today {
        if let (Some(a), Some(b)) = (value_at(&series, d), value_at(&series, d + Duration::days(7))) {
            weekly.push((b - a).max(0) as f64 / 7.0);
        }
        d += Duration::days(7);
    }
    let (best, worst) = (weekly.iter().copied().fold(f64::MIN, f64::max), weekly.iter().copied().fold(f64::MAX, f64::min));
    if weekly.len() >= 3 {
        out.eta_early = if best > 0.0 { eta_for(best) } else { None };
        out.eta_late = if worst > 0.0 { eta_for(worst) } else { None };
    }
    let mean = if weekly.is_empty() { 0.0 } else { weekly.iter().sum::<f64>() / weekly.len() as f64 };
    let spread = if mean != 0.0 { (best - worst) / mean } else { 9.0 };
    out.confidence = Some(if span >= 28 && weekly.len() >= 3 && spread < 1.0 {
        "high"
    } else if span >= 14 {
        "medium"
    } else {
        "low"
    });
    if out.eta.is_none() {
        out.status = "too_far";
        out.reason = Some(msg("At the current pace the finish would take more than 5 years – no date is given.", &[]));
        return out;
    }
    out.status = "ok";
    out.reason = Some(msg(
        "Pace {pace} lines/day over the last {span} days ({days} days with a record). The estimate changes with the pace of work.",
        &[("pace", json!(format!("{pace:.0}"))), ("span", json!(span)), ("days", json!(days_with_data))],
    ));
    out
}

// ---------------------------------------------------------------- weekly goals

type Daily = Vec<(NaiveDate, i64)>;

/// Last known value of every calendar day for one column of the points (1 = translated, 3 = proofread).
fn daily(points: &[Point], idx: usize) -> Daily {
    let mut by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for p in points {
        by_day.insert(p.0.date(), if idx == 1 { p.1 } else { p.3 });
    }
    by_day.into_iter().collect()
}

fn at(series: &Daily, day: NaiveDate) -> Option<i64> {
    series.iter().take_while(|(d, _)| *d <= day).last().map(|(_, v)| *v)
}

/// Monday of the week of `day`.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_monday() as i64)
}

/// Lines added in each of the last `weeks` COMPLETE weeks (Mon-Sun) that the history covers, oldest first.
pub fn weekly_deltas(series: &Daily, today: NaiveDate, weeks: i64) -> Vec<i64> {
    let monday = week_start(today);
    (1..=weeks)
        .rev()
        .filter_map(|i| {
            let start = monday - Duration::days(7 * i);
            let (a, b) = (at(series, start - Duration::days(1))?, at(series, start + Duration::days(6))?);
            Some((b - a).max(0))
        })
        .collect()
}

fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

/// A goal a bit above the recent weekly pace (+10 %, rounded up to tens); `None` when there is too little history.
pub fn auto_goal(deltas: &[i64], remaining: i64) -> Option<i64> {
    if deltas.len() < 2 {
        return None;
    }
    let avg = deltas.iter().sum::<i64>() as f64 / deltas.len() as f64;
    if avg <= 0.0 {
        return None;
    }
    let goal = 10.max(ceil_div((avg * 1.1 + 0.999) as i64, 10) * 10);
    (remaining > 0).then(|| 1.max(goal.min(remaining)))
}

fn goal_status(done: i64, goal: i64, today: NaiveDate) -> Vec<(&'static str, Value)> {
    let days_left = 7 - today.weekday().num_days_from_monday() as i64; // today included
    let elapsed = (7 - days_left + 1) as f64 / 7.0;
    let status = if done >= goal {
        "met"
    } else if done as f64 >= goal as f64 * elapsed * 0.9 {
        "on_track"
    } else {
        "behind"
    };
    vec![
        ("status", json!(status)),
        ("left", json!((goal - done).max(0))),
        ("days_left", json!(days_left)),
        ("needed_per_day", json!(ceil_div((goal - done).max(0), days_left.max(1)))),
    ]
}

/// This week's progress against a goal, for translating and for proofreading. `cfg` = {"translate_per_week": N, "proofread_per_week": N}; 0 / missing = automatic.
pub fn goals(points: &[Point], remaining_translate: i64, remaining_proof: i64, cfg: Option<&Value>, today: NaiveDate) -> Value {
    let monday = week_start(today);
    let mut out = Map::new();
    out.insert("week_start".into(), json!(monday.format("%Y-%m-%d").to_string()));
    for (key, idx, remaining, name) in
        [("translate", 1usize, remaining_translate, "translate_per_week"), ("proofread", 3, remaining_proof, "proofread_per_week")]
    {
        let series = daily(points, idx);
        let mut base = at(&series, monday - Duration::days(1));
        if base.is_none() && !series.is_empty() {
            base = Some(series[0].1);
        }
        let now = series.last().map_or(0, |s| s.1);
        let done = base.map_or(0, |b| (now - b).max(0));
        let deltas = weekly_deltas(&series, today, 4);
        let pace = (!deltas.is_empty()).then(|| round_half_even(deltas.iter().sum::<i64>() as f64 / deltas.len() as f64) as i64);
        let manual = cfg.and_then(|c| c.get(name)).and_then(|v| v.as_f64()).map_or(0, |f| f as i64);
        let goal = if manual > 0 { Some(manual) } else { auto_goal(&deltas, remaining) };
        let mut part = Map::new();
        part.insert("done".into(), json!(done));
        part.insert("goal".into(), json!(goal));
        part.insert("auto".into(), json!(manual <= 0));
        part.insert("pace_per_week".into(), json!(pace));
        match goal.filter(|g| *g != 0) {
            Some(g) => {
                for (k, v) in goal_status(done, g, today) {
                    part.insert(k.into(), v);
                }
                part.insert("percent".into(), json!(round_digits((done as f64 / g as f64 * 100.0).min(100.0), 1)));
            }
            None => {
                part.insert("status".into(), json!("no_goal"));
                part.insert("left".into(), json!(0));
                part.insert("days_left".into(), json!(7 - today.weekday().num_days_from_monday()));
                part.insert("needed_per_day".into(), json!(0));
                part.insert("percent".into(), json!(0));
            }
        }
        out.insert(key.into(), Value::Object(part));
    }
    Value::Object(out)
}
