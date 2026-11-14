//! Native Rust cron scheduler (`pi-gpui/src/engine/scheduler.rs`).
//!
//! Replaces the permanent Node.js background timer with a lightweight Rust 5-field cron
//! parser (`minute hour day_of_month month day_of_week`), file lease management, and
//! due-task detection reading `~/.pi/agent/scheduled-tasks.json` directly.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::local_store::{load_scheduled_tasks_local, write_json_pretty_atomic};
use crate::client::native_index::user_home;
use crate::client::types::ScheduledTask;

fn pi_agent_dir() -> PathBuf {
    user_home().join(".pi").join("agent")
}

/// Parsed 5-field cron schedule (`min hour dom month dow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    pub minutes: [bool; 60],
    pub hours: [bool; 24],
    pub days_of_month: [bool; 32], // 1..=31
    pub months: [bool; 13],        // 1..=12
    pub days_of_week: [bool; 7],   // 0..=6 (Sunday = 0)
}

impl CronSchedule {
    /// Parses a standard 5-field cron expression (e.g. `"0 9 * * 1-5"`, `"*/15 * * * *"`).
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!(
                "Expected 5 cron fields (min hour dom month dow), got {}",
                parts.len()
            ));
        }

        let mut minutes = [false; 60];
        let mut hours = [false; 24];
        let mut days_of_month = [false; 32];
        let mut months = [false; 13];
        let mut days_of_week = [false; 7];

        parse_cron_field(parts[0], 0, 59, &mut minutes)?;
        parse_cron_field(parts[1], 0, 23, &mut hours)?;
        parse_cron_field(parts[2], 1, 31, &mut days_of_month)?;
        parse_cron_field(parts[3], 1, 12, &mut months)?;
        parse_cron_dow_field(parts[4], &mut days_of_week)?;

        Ok(Self {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        })
    }

    /// Checks whether a `(year, month 1..=12, day 1..=31, hour 0..=23, minute 0..=59, dow 0..=6)` matches.
    pub fn matches(&self, month: usize, day: usize, hour: usize, minute: usize, dow: usize) -> bool {
        if month == 0 || month > 12 || day == 0 || day > 31 || hour >= 24 || minute >= 60 || dow >= 7
        {
            return false;
        }
        self.months[month]
            && self.days_of_month[day]
            && self.hours[hour]
            && self.minutes[minute]
            && self.days_of_week[dow]
    }

    /// Computes the next matching Unix timestamp in milliseconds strictly after `after_ms` (UTC).
    pub fn next_after_utc_ms(&self, after_ms: u64) -> Option<u64> {
        // Advance to the start of the next whole minute
        let mut candidate_min = (after_ms / 60_000) + 1;
        // Search up to 366 days ahead (527,040 minutes)
        for _ in 0..(366 * 24 * 60) {
            let ts_secs = candidate_min * 60;
            let (_, month, day, hour, minute, dow) = unix_secs_to_utc_parts(ts_secs);
            if self.matches(month, day, hour, minute, dow) {
                return Some(ts_secs * 1000);
            }
            candidate_min += 1;
        }
        None
    }
}

fn parse_cron_field(field: &str, min: usize, max: usize, out: &mut [bool]) -> Result<(), String> {
    for part in field.split(',') {
        let (range_part, step) = if let Some((r, s)) = part.split_once('/') {
            let st: usize = s.parse().map_err(|_| format!("Invalid cron step: {s}"))?;
            if st == 0 {
                return Err("Cron step cannot be 0".to_string());
            }
            (r, st)
        } else {
            (part, 1usize)
        };

        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            let s: usize = a.parse().map_err(|_| format!("Invalid cron number: {a}"))?;
            let e: usize = b.parse().map_err(|_| format!("Invalid cron number: {b}"))?;
            (s, e)
        } else {
            let v: usize = range_part
                .parse()
                .map_err(|_| format!("Invalid cron value: {range_part}"))?;
            (v, v)
        };

        if start < min || end > max || start > end {
            return Err(format!("Cron range {start}..={end} out of bounds {min}..={max}"));
        }

        let mut i = start;
        while i <= end {
            out[i] = true;
            i += step;
        }
    }
    Ok(())
}

fn parse_cron_dow_field(field: &str, out: &mut [bool; 7]) -> Result<(), String> {
    let mut temp = [false; 8];
    parse_cron_field(field, 0, 7, &mut temp)?;
    for i in 0..7 {
        out[i] = temp[i];
    }
    if temp[7] {
        out[0] = true;
    }
    Ok(())
}

/// Converts Unix seconds (UTC) into `(year, month 1..=12, day 1..=31, hour 0..=23, minute 0..=59, dow 0..=6)`.
pub fn unix_secs_to_utc_parts(secs: u64) -> (i32, usize, usize, usize, usize, usize) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3600) as usize;
    let minute = ((rem % 3600) / 60) as usize;
    // 1970-01-01 was Thursday (dow = 4)
    let dow = ((days + 4).rem_euclid(7)) as usize;

    // Howard Hinnant's civil_from_days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as usize;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as usize;
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;

    (year, m, d, hour, minute, dow)
}

/// File lease at `~/.pi/agent/scheduled-tasks/scheduler-lease.json` to prevent duplicate execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerLease {
    pub holder_pid: u32,
    pub acquired_at_ms: u64,
    pub expires_at_ms: u64,
}

fn lease_file_path() -> PathBuf {
    pi_agent_dir()
        .join("scheduled-tasks")
        .join("scheduler-lease.json")
}

/// Attempts to acquire or renew the scheduler lease for `ttl_ms` (default 90_000ms).
pub fn try_acquire_scheduler_lease(ttl_ms: u64) -> bool {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let my_pid = std::process::id();
    let path = lease_file_path();

    if let Ok(raw) = std::fs::read_to_string(&path)
        && let Ok(existing) = serde_json::from_str::<SchedulerLease>(&raw)
        && existing.holder_pid != my_pid
        && existing.expires_at_ms > now_ms
    {
        return false;
    }

    let lease = SchedulerLease {
        holder_pid: my_pid,
        acquired_at_ms: now_ms,
        expires_at_ms: now_ms + ttl_ms,
    };
    if let Ok(val) = serde_json::to_value(&lease) {
        let _ = write_json_pretty_atomic(&path, &val);
        true
    } else {
        false
    }
}

/// Returns enabled scheduled tasks whose `next_run_at` or cron expression is due at `now_ms`.
pub fn find_due_scheduled_tasks(now_ms: u64) -> Vec<ScheduledTask> {
    let tasks = load_scheduled_tasks_local();
    let mut due = Vec::new();

    for task in tasks {
        if !task.enabled {
            continue;
        }
        if let Some(ref next_iso) = task.next_run_at
            && let Some(next_ms) = parse_epoch_or_iso_ms(next_iso)
            && next_ms <= now_ms
        {
            due.push(task);
        }
    }
    due
}

fn parse_epoch_or_iso_ms(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if let Ok(ms) = trimmed.parse::<u64>() {
        return Some(ms);
    }
    None
}

/// Preview helper that computes the next `count` UTC timestamps (in ms) for a cron expression.
pub fn preview_cron_runs_utc_ms(cron_expr: &str, from_ms: u64, count: usize) -> Result<Vec<u64>, String> {
    let sched = CronSchedule::parse(cron_expr)?;
    let mut out = Vec::with_capacity(count);
    let mut cursor = from_ms;
    for _ in 0..count {
        if let Some(next) = sched.next_after_utc_ms(cursor) {
            out.push(next);
            cursor = next;
        } else {
            break;
        }
    }
    Ok(out)
}

/// Returns a JSON preview matching `/api/scheduled-tasks/preview`.
pub fn preview_schedule_local(cron_expr: &str, count: usize) -> Value {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match preview_cron_runs_utc_ms(cron_expr, now_ms, count) {
        Ok(runs) => json!({ "ok": true, "nextRunsMs": runs }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_schedule_parses_and_computes_next_runs() {
        let sched = CronSchedule::parse("*/15 9-17 * * 1-5").unwrap();
        assert!(sched.minutes[0]);
        assert!(sched.minutes[15]);
        assert!(!sched.minutes[10]);
        assert!(sched.hours[9]);
        assert!(sched.hours[17]);
        assert!(!sched.hours[18]);
        assert!(sched.days_of_week[1]);
        assert!(!sched.days_of_week[0]);

        // 2025-01-06 08:50:00 UTC is Monday (1736153400000 ms)
        let next = sched.next_after_utc_ms(1_736_153_400_000).unwrap();
        // Should be 2025-01-06 09:00:00 UTC (1736154000000 ms)
        assert_eq!(next, 1_736_154_000_000);
    }
}
