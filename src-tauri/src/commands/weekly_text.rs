//! The weekly summary's words, written by the app itself — used when the AI summary is
//! switched off (or chosen against). Same `Narrative` shape as the model's, so the report
//! page renders either. Every sentence comes straight from `WeekMetrics`: plain
//! comparisons with "your usual", never a forecast (see CLAUDE.md, "Nothing predicts
//! forward"). The report already shows the scorecard and detail tables, so this sticks
//! to what's worth reading in a sentence.

use super::weekly::{Correlation, DayNote, NarrSection, Narrative, ScoreRow, WeekMetrics};
use chrono::NaiveDate;

/// Recorded in `weekly_summaries.model` for a summary written this way.
pub const BUILT_IN: &str = "built-in";

fn num(v: f64, decimals: u8) -> String {
    let s = format!("{:.*}", decimals as usize, v);
    if decimals == 0 {
        // Thousands separators for steps.
        let neg = s.starts_with('-');
        let digits: Vec<char> = s.trim_start_matches('-').chars().collect();
        let mut out = String::new();
        for (i, c) in digits.iter().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(*c);
        }
        return if neg { format!("-{}", out) } else { out };
    }
    s
}

fn with_unit(v: f64, row: &ScoreRow) -> String {
    match row.unit.as_str() {
        "" => num(v, row.decimals),
        u if u.starts_with('/') => format!("{}{}", num(v, row.decimals), u),
        u => format!("{} {}", num(v, row.decimals), u),
    }
}

/// "Tue 6 Oct" for an ISO date; anything after the date ("2026-10-06 (1 of 3)") is kept.
fn day(date: &str) -> String {
    let (d, rest) = date.split_at(date.len().min(10));
    NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map(|x| format!("{}{}", x.format("%a %-d %b"), rest))
        .unwrap_or_else(|_| date.to_string())
}

/// The first `n` items, then "and N more".
fn some_of(items: &[String], n: usize) -> String {
    if items.len() <= n {
        return items.join(", ");
    }
    format!("{} and {} more", items[..n].join(", "), items.len() - n)
}

/// Lower-case a label for mid-sentence use, but leave acronyms ("BP") alone.
fn mid_sentence(label: &str) -> String {
    label
        .split(' ')
        .enumerate()
        .map(|(i, w)| {
            let acronym = w.chars().filter(|c| c.is_alphabetic()).count() > 1 && w.chars().all(|c| !c.is_lowercase());
            if acronym || i > 0 { w.to_string() } else { w.to_lowercase() }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn row<'a>(m: &'a WeekMetrics, key: &str) -> Option<&'a ScoreRow> {
    m.scorecard.iter().find(|r| r.key == key && r.this_week.is_some())
}

/// "Fatigue averaged 5.2/10 — above your usual 4.1 (last week 4.6)."
fn compare(r: &ScoreRow) -> Option<String> {
    let v = r.this_week?;
    let mut s = format!("{} averaged {}", r.label, with_unit(v, r));
    match (&r.flag, r.baseline) {
        (Some(f), Some(b)) => s.push_str(&format!(
            " — {}{} your usual {}",
            if r.strong { "well " } else { "" },
            if f == "higher" { "above" } else { "below" },
            with_unit(b, r)
        )),
        (None, Some(b)) => s.push_str(&format!(", close to your usual {}", with_unit(b, r))),
        _ => {}
    }
    if let Some(l) = r.last_week {
        s.push_str(&format!(" (last week {})", with_unit(l, r)));
    }
    if r.n_days < 4 {
        s.push_str(&format!(", from only {} day{}", r.n_days, if r.n_days == 1 { "" } else { "s" }));
    }
    s.push('.');
    Some(s)
}

/// Good/bad/neutral for a section, from its flagged rows and which way is worse.
fn severity(rows: &[&ScoreRow]) -> String {
    let mut worse = false;
    let mut better = false;
    for r in rows {
        if let (Some(f), Some(hiw)) = (&r.flag, r.higher_is_worse) {
            if (f == "higher") == hiw { worse = true } else { better = true }
        }
    }
    match (worse, better) {
        (true, _) => "warning",
        (false, true) => "positive",
        _ => "neutral",
    }
    .to_string()
}

fn section(title: &str, icon: &str, severity: String, points: Vec<String>) -> NarrSection {
    NarrSection { title: title.into(), icon: icon.into(), severity, points }
}

fn day_line(label: &str, d: &DayNote) -> String {
    let mut s = format!("{}: {}, fatigue {}/10", label, day(&d.date), num(d.fatigue, 0));
    if let Some(desc) = d.fatigue_desc.as_deref().map(|t| t.trim().trim_end_matches(['.', ' '])).filter(|t| !t.is_empty()) {
        s.push_str(&format!(" — \"{}\"", desc));
    }
    s.push('.');
    let mut before = Vec::new();
    if let Some(v) = d.prev_day_sleep_score { before.push(format!("sleep score {}", num(v, 1))); }
    if let Some(v) = d.prev_day_hours_asleep { before.push(format!("{} h asleep", num(v, 1))); }
    if let Some(v) = d.prev_day_steps { before.push(format!("{} steps", num(v, 0))); }
    if let Some(v) = d.prev_day_activity_hours { before.push(format!("{} h of logged activity", num(v, 1))); }
    if !before.is_empty() {
        s.push_str(&format!(" The day before: {}.", before.join(", ")));
    }
    if !d.exposures.is_empty() {
        s.push_str(&format!(" Exposures that day: {}.", some_of(&d.exposures, 4)));
    }
    s
}

fn correlation_line(c: &Correlation) -> String {
    let when = if c.lag_days == 0 { "on days with" } else { "the day after" };
    let more = if c.r > 0.0 { "higher" } else { "lower" };
    let strength = if c.r.abs() >= 0.5 { "fairly consistently" } else { "somewhat" };
    format!(
        "Over the last 12 weeks, fatigue was {} {} {} more {} (r = {:.2}, {} days). An association, not a cause.",
        strength, more, when, c.measure, c.r, c.n
    )
}

pub fn narrative(m: &WeekMetrics) -> Narrative {
    let c = &m.completeness;
    let fatigue = row(m, "fatigue");

    // ── Headline ──
    let headline = match fatigue.and_then(|r| r.this_week.map(|v| (r, v))) {
        Some((r, v)) => {
            let vs = match (&r.flag, r.baseline) {
                (Some(f), Some(b)) => format!(
                    ", {}{} your usual {}",
                    if r.strong { "well " } else { "" },
                    if f == "higher" { "above" } else { "below" },
                    with_unit(b, r)
                ),
                (None, Some(_)) => ", about your usual".to_string(),
                _ => String::new(),
            };
            let days = match (m.bad_days, m.good_days) {
                (0, 0) => String::new(),
                (b, 0) => format!(", with {} bad day{}", b, if b == 1 { "" } else { "s" }),
                (0, g) => format!(", with {} good day{}", g, if g == 1 { "" } else { "s" }),
                (b, g) => format!(", with {} bad and {} good day{}", b, g, if g == 1 { "" } else { "s" }),
            };
            format!("Fatigue averaged {}{}{}.", with_unit(v, r), vs, days)
        }
        None => format!("{} day{} logged this week, without a fatigue rating.", c.days_in_week, if c.days_in_week == 1 { "" } else { "s" }),
    };

    // ── Overview ──
    let mut overview = Vec::new();
    let flagged: Vec<String> = m
        .scorecard
        .iter()
        .filter_map(|r| {
            let (f, v, b) = (r.flag.as_ref()?, r.this_week?, r.baseline?);
            Some(format!(
                "{} {}{} ({} vs {})",
                mid_sentence(&r.label),
                if r.strong { "well " } else { "" },
                if f == "higher" { "higher" } else { "lower" },
                with_unit(v, r),
                with_unit(b, r)
            ))
        })
        .collect();
    if flagged.is_empty() {
        overview.push("Every measure was within your usual range for the past 8 weeks.".to_string());
    } else {
        overview.push(format!("Outside your usual range: {}.", flagged.join(", ")));
    }
    if c.fatigue_days < 5 {
        overview.push(format!(
            "Only {} of 7 days had a fatigue rating, so this is a partial picture.",
            c.fatigue_days
        ));
    }

    let mut sections = Vec::new();

    // ── Fatigue & symptoms ──
    {
        let rows: Vec<&ScoreRow> = ["fatigue", "headache"].iter().filter_map(|k| row(m, k)).collect();
        let mut p: Vec<String> = rows.iter().filter_map(|r| compare(r)).collect();
        if let Some(d) = &m.worst_day {
            p.push(day_line("Hardest day", d));
        }
        if let Some(d) = m.best_day.as_ref().filter(|b| m.worst_day.as_ref().map(|w| w.date != b.date).unwrap_or(true)) {
            p.push(day_line("Best day", d));
        }
        sections.push(section("Fatigue & symptoms", "🔋", severity(&rows), p));
    }

    // ── Sleep ──
    {
        let rows: Vec<&ScoreRow> = ["sleep", "asleep"].iter().filter_map(|k| row(m, k)).collect();
        let p = rows.iter().filter_map(|r| compare(r)).collect();
        sections.push(section("Sleep & rest", "🌙", severity(&rows), p));
    }

    // ── Activity ──
    {
        let rows: Vec<&ScoreRow> = ["steps", "load", "load_hours"].iter().filter_map(|k| row(m, k)).collect();
        let mut p: Vec<String> = rows.iter().filter_map(|r| compare(r)).collect();
        let top: Vec<String> = m
            .activity_by_category
            .iter()
            .filter(|(_, h)| *h > 0.0)
            .take(4)
            .map(|(n, h)| format!("{} {} h", n, num(*h, 1)))
            .collect();
        if !top.is_empty() {
            p.push(format!("Most logged time went on: {}.", top.join(", ")));
        }
        // Activity is neutral either way: more isn't better or worse in itself.
        sections.push(section("Activity & pacing", "📈", "neutral".into(), p));
    }

    // ── Heart & blood pressure ──
    {
        let rows: Vec<&ScoreRow> = ["rhr", "sys", "dia", "alcohol"].iter().filter_map(|k| row(m, k)).collect();
        let p = rows.iter().filter_map(|r| compare(r)).collect();
        sections.push(section("Heart, blood pressure & alcohol", "❤️", severity(&rows), p));
    }

    // ── Food & exposures ──
    {
        let f = &m.food;
        let mut p = Vec::new();
        if !f.top_items.is_empty() {
            let top: Vec<String> = f.top_items.iter().take(6).map(|(n, k)| format!("{} ({}×)", n, k)).collect();
            p.push(format!("Most often had: {}.", top.join(", ")));
        }
        if !f.flag_days.is_empty() {
            // Flags present on every food day read better as one group.
            let all = c.food_days.max(1);
            let every: Vec<String> = f.flag_days.iter().filter(|(_, d)| *d >= all).map(|(n, _)| n.clone()).collect();
            let some: Vec<String> = f.flag_days.iter().filter(|(_, d)| *d < all).map(|(n, d)| format!("{} on {}", n, d)).collect();
            let mut parts = Vec::new();
            if !every.is_empty() {
                parts.push(format!("every day: {}", every.join(", ")));
            }
            if !some.is_empty() {
                parts.push(format!("{} of {} days", some.join(", "), all));
            }
            p.push(format!("By flag — {}.", parts.join("; ")));
        }
        if !f.new_items.is_empty() {
            p.push(format!("New this week: {}.", some_of(&f.new_items, 8)));
        }
        if !f.worst_day_items.is_empty() {
            if let Some(w) = &m.worst_day {
                p.push(format!("On the hardest day ({}): {}.", day(&w.date), some_of(&f.worst_day_items, 8)));
            }
        }
        if !m.exposures.is_empty() {
            // Stored as "YYYY-MM-DD: name", one per day it was logged; count days per name.
            let mut counts: Vec<(String, usize)> = Vec::new();
            for e in &m.exposures {
                let name = e.split_once(": ").map(|(_, n)| n).unwrap_or(e).trim().to_string();
                match counts.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, k)) => *k += 1,
                    None => counts.push((name, 1)),
                }
            }
            counts.sort_by(|a, b| b.1.cmp(&a.1));
            let list: Vec<String> = counts
                .iter()
                .map(|(n, k)| if *k > 1 { format!("{} ({} days)", n, k) } else { n.clone() })
                .collect();
            p.push(format!("Exposures noted: {}.", some_of(&list, 6)));
        }
        sections.push(section("Food & exposures", "🍽️", "neutral".into(), p));
    }

    // ── Medication ──
    {
        let md = &m.meds;
        let mut p: Vec<String> = md.changes.iter().map(|l| format!("{}.", l.trim_end_matches('.'))).collect();
        let expected: i64 = md.adherence.iter().map(|a| a.expected_doses).sum();
        let logged: i64 = md.adherence.iter().map(|a| a.logged_doses).sum();
        if expected > 0 {
            p.push(format!("{} of {} scheduled doses were logged on the days you logged medication.", logged.min(expected), expected));
        }
        let gaps: Vec<String> = md
            .adherence
            .iter()
            .filter(|a| a.missed_doses > 0)
            .map(|a| format!("{} on {}", a.name, a.days_missed.iter().map(|d| day(d)).collect::<Vec<_>>().join(", ")))
            .collect();
        if !gaps.is_empty() {
            p.push(format!("No dose was logged for: {}.", gaps.join("; ")));
        }
        if !md.days_without_any_dose.is_empty() && md.days_with_dose > 0 {
            p.push(format!(
                "Nothing at all was logged on {} — unknown rather than missed.",
                md.days_without_any_dose.iter().map(|d| day(d)).collect::<Vec<_>>().join(", ")
            ));
        }
        sections.push(section("Medications", "💊", "neutral".into(), p));
    }

    // ── Records ──
    {
        // The full list is in the report's Records table; here, a count and anything flagged.
        let mut p = Vec::new();
        let labs = &m.new_lab_results;
        if !labs.is_empty() {
            let mut dates: Vec<String> = labs.iter().map(|l| l.date.clone()).collect();
            dates.sort();
            dates.dedup();
            let flagged = labs.iter().filter(|l| l.flag.is_some()).count();
            p.push(format!(
                "{} new lab result{} ({}){}.",
                labs.len(),
                if labs.len() == 1 { "" } else { "s" },
                dates.iter().map(|d| day(d)).collect::<Vec<_>>().join(", "),
                match flagged {
                    0 => ", none flagged outside its reference range".to_string(),
                    k => format!(", {} flagged", k),
                }
            ));
            for l in labs.iter().filter(|l| l.flag.is_some()) {
                let mut s = format!("{}: {} ({})", l.test, l.value, l.flag.as_deref().unwrap_or("").to_lowercase());
                if let Some(prev) = &l.previous {
                    s.push_str(&format!(", previously {}", prev));
                }
                s.push('.');
                p.push(s);
            }
        }
        if !m.vault_changes.is_empty() {
            p.push(format!(
                "{} note{} added or edited in your records.",
                m.vault_changes.len(),
                if m.vault_changes.len() == 1 { "" } else { "s" }
            ));
        }
        let sev = if m.new_lab_results.iter().any(|l| l.flag.is_some()) { "warning" } else { "neutral" };
        sections.push(section("Records & results", "🧪", sev.into(), p));
    }

    // ── Patterns ──
    {
        let p = m.correlations.iter().take(3).map(correlation_line).collect();
        sections.push(section("Patterns over 12 weeks", "🔍", "neutral".into(), p));
    }

    // Only flagged results go here: anything stronger is a clinician's call, not the app's.
    let to_raise = m
        .new_lab_results
        .iter()
        .filter(|l| l.flag.is_some())
        .map(|l| format!("{} {} on {} was flagged {}.", l.test, l.value, day(&l.date), l.flag.as_deref().unwrap_or("").to_lowercase()))
        .collect();

    Narrative { headline, overview: overview.join(" "), sections, to_raise }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::weekly::{Completeness, LabItem};

    fn score(key: &str, label: &str, unit: &str, this: f64, base: f64, flag: Option<&str>, hiw: Option<bool>) -> ScoreRow {
        ScoreRow {
            key: key.into(),
            label: label.into(),
            unit: unit.into(),
            decimals: if unit == "" { 0 } else { 1 },
            higher_is_worse: hiw,
            this_week: Some(this),
            last_week: Some(base),
            baseline: Some(base),
            flag: flag.map(Into::into),
            strong: false,
            n_days: 7,
            history: vec![],
        }
    }

    #[test]
    fn writes_a_plain_summary_from_the_numbers() {
        let m = WeekMetrics {
            week_start: "2026-10-05".into(),
            week_end: "2026-10-11".into(),
            completeness: Completeness { days_in_week: 7, fatigue_days: 7, food_days: 7, ..Default::default() },
            scorecard: vec![
                score("fatigue", "Fatigue", "/10", 6.2, 4.9, Some("higher"), Some(true)),
                score("sleep", "Sleep score", "/10", 6.0, 6.1, None, Some(false)),
                score("steps", "Steps", "", 5321.0, 7000.0, Some("lower"), None),
            ],
            bad_days: 2,
            good_days: 0,
            new_lab_results: vec![LabItem { test: "CRP".into(), date: "2026-10-07".into(), value: "12 mg/L".into(), flag: Some("HIGH".into()), ..Default::default() }],
            ..Default::default()
        };
        let n = narrative(&m);
        assert_eq!(n.headline, "Fatigue averaged 6.2/10, above your usual 4.9/10, with 2 bad days.");
        assert!(n.overview.contains("fatigue higher (6.2/10 vs 4.9/10), steps lower (5,321 vs 7,000)"), "{}", n.overview);
        let fatigue = &n.sections[0];
        assert_eq!(fatigue.severity, "warning");
        assert!(fatigue.points[0].starts_with("Fatigue averaged 6.2/10 — above your usual 4.9/10"));
        let activity = n.sections.iter().find(|s| s.title.starts_with("Activity")).unwrap();
        assert!(activity.points[0].contains("Steps averaged 5,321"), "{:?}", activity.points);
        assert_eq!(n.to_raise, vec!["CRP 12 mg/L on Wed 7 Oct was flagged high."]);
        let records = n.sections.iter().find(|s| s.title.starts_with("Records")).unwrap();
        assert_eq!(records.points[0], "1 new lab result (Wed 7 Oct), 1 flagged.");
    }

    #[test]
    fn small_helpers() {
        assert_eq!(mid_sentence("Systolic BP"), "systolic BP");
        assert_eq!(mid_sentence("Resting heart rate"), "resting heart rate");
        assert_eq!(day("2026-10-06 (1 of 3)"), "Tue 6 Oct (1 of 3)");
        let items: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(some_of(&items, 2), "a, b and 1 more");
    }

    #[test]
    fn a_week_without_ratings_still_gets_a_headline() {
        let m = WeekMetrics { completeness: Completeness { days_in_week: 3, ..Default::default() }, ..Default::default() };
        let n = narrative(&m);
        assert_eq!(n.headline, "3 days logged this week, without a fatigue rating.");
        assert!(n.overview.contains("partial picture"));
    }
}
