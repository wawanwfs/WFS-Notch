//! Local-only, bounded RFC 5545 iCalendar interchange.
//! UTC, IANA TZID, and explicit user-selected floating timezones are resolved to UTC.
//! Bounded DAILY/WEEKLY/MONTHLY/YEARLY RRULEs are expanded into local instances.
//! Source recurrence metadata is retained with an explicit version; RFC5545
//! Scoped RECURRENCE-ID overrides are accepted only when the original master
//! occurrence, identical source timezone and resulting collision-free event
//! can be proven. Scoped cancellations require the same proof and preview.
use crate::{
    all_day::{parse_ics_date, AllDaySpan},
    ics_time::CalendarStart,
    new_id, now_ms, remember_calendar_cancellation, validate_event, validate_timed_end, with_store,
    AppData, CalendarEvent, Store,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Mutex,
};
use tauri::State;

const MAX_ICS_BYTES: usize = 256 * 1024;
const MAX_IMPORT_EVENTS: usize = 250;
const LOCAL_UID_SUFFIX: &str = "@wfs-notch.local";

#[derive(Default)]
struct PartialEvent {
    title: Option<String>,
    description: String,
    start: Option<CalendarStart>,
    start_property: Option<(String, String)>,
    all_day_start: Option<NaiveDate>,
    end_property: Option<(String, String)>,
    duration_property: Option<String>,
    uid: Option<String>,
    reminder: Option<u32>,
    has_alarm: bool,
    recurrence: Option<String>,
    exdates: Vec<(String, String)>,
    rdates: Vec<(String, String)>,
    recurrence_id: Option<(String, String)>,
    status: Option<String>,
}

/// A source exception remains linked to its *original* recurrence timestamp.
/// Source exceptions never share generated standalone event IDs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct SourceOccurrenceOverride {
    recurrence_id: (String, String),
    dtstart: (String, String),
    dtend: Option<(String, String)>,
    duration: Option<String>,
    title: String,
    description: String,
    reminder_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct SourceOccurrenceCancellation {
    recurrence_id: (String, String),
    occurrence_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct SeriesSource {
    // Persist the original normalized source without reconstructing it from
    // flattened occurrences, which may have individual user edits.
    pub schema_version: u8,
    pub uid: String,
    pub title: String,
    pub description: String,
    pub dtstart: (String, String),
    pub dtend: Option<(String, String)>,
    pub duration: Option<String>,
    pub rrule: Option<String>,
    pub exdates: Vec<(String, String)>,
    pub rdates: Vec<(String, String)>,
    pub floating_timezone: Option<String>,
    // Backward-compatible: Phase17 stored no source alarm; a legacy source
    // with reminders will fall back to flattened export, never lose alarms.
    #[serde(default)]
    pub reminder_minutes: Option<u32>,
    // Explicit overrides are tracked, never silently replaced on reimport.
    #[serde(default)]
    pub modified_occurrence_ids: HashSet<String>,
    // Legacy version-1 metadata has no exceptions. Never infer them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<SourceOccurrenceOverride>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) cancellations: Vec<SourceOccurrenceCancellation>,
}
impl SeriesSource {
    pub(super) fn valid(&self, id: &str) -> bool {
        self.schema_version == 1
            && id == self.uid
            && id.starts_with("ics:")
            && id.len() <= 180
            && !id.chars().any(char::is_control)
            && !self.title.trim().is_empty()
            && self.title.chars().count() <= 200
            && self.description.chars().count() <= 2000
            && self.dtstart.0.starts_with("DTSTART")
            && self.dtstart.0.len() <= 110
            && self.dtstart.1.len() <= 80
            && self.rrule.as_ref().is_none_or(|rule| rule.len() <= 512)
            && self.exdates.len() <= 250
            && self.rdates.len() <= 250
            && matches!(self.reminder_minutes, None | Some(0 | 5 | 15 | 30 | 60))
            && self.modified_occurrence_ids.len() <= 250
            && self
                .overrides
                .len()
                .saturating_add(self.cancellations.len())
                <= 250
            && self.cancellations.iter().all(|exception| {
                exception.recurrence_id.0.starts_with("RECURRENCE-ID")
                    && exception.recurrence_id.0.len() <= 110
                    && exception.recurrence_id.1.len() <= 80
                    && exception
                        .occurrence_id
                        .strip_prefix(id)
                        .and_then(|suffix| suffix.strip_prefix(':'))
                        .is_some_and(|timestamp| timestamp.parse::<u64>().is_ok())
            })
            && self.overrides.iter().all(|exception| {
                exception.recurrence_id.0.starts_with("RECURRENCE-ID")
                    && exception.recurrence_id.0.len() <= 110
                    && exception.recurrence_id.1.len() <= 80
                    && exception.dtstart.0.starts_with("DTSTART")
                    && exception.dtstart.0.len() <= 110
                    && exception.dtstart.1.len() <= 80
                    && exception
                        .dtend
                        .as_ref()
                        .is_none_or(|(key, value)| key.len() <= 110 && value.len() <= 80)
                    && exception
                        .duration
                        .as_ref()
                        .is_none_or(|value| value.len() <= 36)
                    && exception.title.chars().count() <= 200
                    && !exception.title.trim().is_empty()
                    && exception.description.chars().count() <= 2000
                    && matches!(
                        exception.reminder_minutes,
                        None | Some(0 | 5 | 15 | 30 | 60)
                    )
            })
            && self.modified_occurrence_ids.iter().all(|key| {
                key.starts_with(id) && key.len() <= 240 && !key.chars().any(char::is_control)
            })
    }
}
#[cfg(test)]
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    }
}
// Howard Hinnant's proleptic Gregorian civil-date algorithms.
#[cfg(test)]
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}
#[cfg(test)]
fn utc_to_ms(value: &str) -> Result<u64, String> {
    let b = value.as_bytes();
    if b.len() != 16
        || b[8] != b'T'
        || b[15] != b'Z'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 8 || i == 15 || c.is_ascii_digit())
    {
        return Err("Only UTC DTSTART:YYYYMMDDTHHMMSSZ is supported; TZID and floating times are not imported".into());
    }
    let part = |a: usize, z: usize| -> Result<i64, String> {
        value[a..z]
            .parse::<i64>()
            .map_err(|_| "Invalid UTC timestamp".to_owned())
    };
    let (y, m, d) = (part(0, 4)?, part(4, 6)?, part(6, 8)?);
    let (h, min, sec) = (part(9, 11)?, part(11, 13)?, part(13, 15)?);
    if !(2000..=2099).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=days_in_month(y, m)).contains(&d)
        || !(0..=23).contains(&h)
        || !(0..=59).contains(&min)
        || !(0..=59).contains(&sec)
    {
        return Err("UTC event timestamp outside supported range".into());
    }
    let secs = days_from_civil(y, m, d) * 86400 + h * 3600 + min * 60 + sec;
    u64::try_from(secs)
        .map(|v| v * 1000)
        .map_err(|e| e.to_string())
}
fn ms_to_utc(ms: u64) -> String {
    let sec = (ms / 1000) as i64;
    let (y, m, d) = civil_from_days(sec.div_euclid(86400));
    let time = sec.rem_euclid(86400);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        time / 3600,
        time / 60 % 60,
        time % 60
    )
}
fn unescape_text(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
fn escape_text(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}
fn folded_line(text: &str) -> String {
    let mut out = String::new();
    let mut column = 0usize;
    for ch in text.chars() {
        let bytes = ch.len_utf8();
        if column + bytes > 75 {
            out.push_str("\r\n ");
            column = 1;
        }
        out.push(ch);
        column += bytes;
    }
    out.push_str("\r\n");
    out
}
fn valid_uid(raw: &str) -> bool {
    !raw.is_empty()
        && raw.len() <= 192
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:@".contains(&b))
}
fn event_id(uid: Option<&str>) -> String {
    match uid {
        Some(value) if valid_uid(value) => {
            if let Some(source_id) = value.strip_suffix(LOCAL_UID_SUFFIX) {
                if valid_uid(source_id) {
                    return source_id.to_owned();
                }
            }
            format!("ics:{value}")
        }
        _ => new_id(),
    }
}
// Parse RFC 5545 EXDATE / RDATE lists as the same kind of start as the event.
// No implicit timezone, no mixed DATE/DATE-TIME, no unbounded exception lists.
fn occurrence_date_values(
    properties: &[(String, String)],
    all_day: bool,
    floating_timezone: Option<&str>,
) -> Result<Vec<u64>, String> {
    let mut result = Vec::new();
    for (key, values) in properties {
        for raw in values.split(',') {
            if raw.is_empty() || result.len() >= MAX_IMPORT_EVENTS {
                return Err(
                    "Calendar exception list contains an empty date or exceeds 250 values".into(),
                );
            }
            let instant = if all_day {
                let date = parse_ics_date(key, raw)?;
                let span =
                    AllDaySpan::new(date, date.succ_opt().ok_or("Calendar date out of range")?)?;
                span.utc_midnight_ms()?
            } else {
                CalendarStart::parse(key, raw, floating_timezone)?
                    .occurrences(None)?
                    .into_iter()
                    .next()
                    .ok_or("Missing occurrence timestamp")?
            };
            result.push(instant);
        }
    }
    Ok(result)
}

// RFC 5545 positive duration subset; exact elapsed duration only.
// Supports PnW, PnD, PnDTnHnMnS and PTnHnMnS, without fractional units.
// Negative, zero, duplicate, out-of-order or unbounded values fail closed.
fn parse_timed_duration(raw: &str) -> Result<u64, String> {
    if raw.len() > 36 || !raw.starts_with('P') || raw.len() < 3 {
        return Err("Invalid or unsupported iCalendar DURATION".into());
    }
    let mut seconds = 0_u64;
    let mut seen = std::collections::HashSet::new();
    let mut in_time = false;
    let mut last_rank = 0_u8;
    let mut digits = String::new();
    let mut had_value = false;
    for ch in raw[1..].chars() {
        if ch == 'T' {
            if in_time || !digits.is_empty() || seen.contains(&'W') {
                return Err("Invalid DURATION time component".into());
            }
            in_time = true;
            last_rank = 0;
            continue;
        }
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() > 9 {
                return Err("DURATION component is too large".into());
            }
            continue;
        }
        let count = digits
            .parse::<u64>()
            .map_err(|_| "Malformed DURATION component")?;
        digits.clear();
        let (unit, rank) = match (in_time, ch) {
            (false, 'W') => (604800_u64, 1),
            (false, 'D') => (86400, 2),
            (true, 'H') => (3600, 1),
            (true, 'M') => (60, 2),
            (true, 'S') => (1, 3),
            _ => return Err("Unsupported DURATION component".into()),
        };
        if !seen.insert(ch)
            || rank <= last_rank
            || (ch == 'W' && seen.len() > 1)
            || (ch == 'D' && seen.contains(&'W'))
        {
            return Err("Duplicate or out-of-order DURATION unit".into());
        }
        last_rank = rank;
        seconds = seconds
            .checked_add(count.checked_mul(unit).ok_or("DURATION overflow")?)
            .ok_or("DURATION overflow")?;
        had_value = true;
    }
    if !digits.is_empty()
        || !had_value
        || seconds == 0
        || seconds > 366 * 86400
        || (in_time && !seen.iter().any(|unit| matches!(unit, 'H' | 'M' | 'S')))
    {
        return Err("DURATION must be positive and at most 366 days".into());
    }
    seconds
        .checked_mul(1000)
        .ok_or_else(|| "DURATION overflow".to_owned())
}

/// Apply an explicit single-occurrence override ONLY if we can establish the
/// original occurrence key from a validated master. All caller mutations are
/// to an in-memory vector; the importer commits the entire calendar atomically.
fn apply_ics_overrides(
    events: &mut Vec<CalendarEvent>,
    sources: &mut [SeriesSource],
    pending: Vec<PartialEvent>,
    floating_timezone: Option<&str>,
) -> Result<(), String> {
    if pending.len() > MAX_IMPORT_EVENTS {
        return Err("More than 250 recurrence overrides".into());
    }
    let mut modified = HashSet::new();
    // Remove canceled occurrences before applying moves so a moved instance
    // may occupy a canceled slot, independent of VEVENT order in the file.
    let (canceled, edited): (Vec<_>, Vec<_>) = pending
        .into_iter()
        .partition(|event| event.status.as_deref() == Some("CANCELLED"));
    for override_event in canceled.into_iter().chain(edited) {
        if override_event.recurrence.is_some()
            || !override_event.exdates.is_empty()
            || !override_event.rdates.is_empty()
        {
            return Err("RECURRENCE-ID cannot carry a nested recurrence or date list".into());
        }
        let uid = override_event
            .uid
            .as_deref()
            .filter(|uid| valid_uid(uid) && uid.len() <= 128 && !uid.ends_with(LOCAL_UID_SUFFIX))
            .ok_or("RECURRENCE-ID requires the exact stable master UID")?;
        let series_key = event_id(Some(uid));
        let source = sources
            .iter_mut()
            .find(|series| series.uid == series_key)
            .ok_or("RECURRENCE-ID has no matching recurrence master")?;
        let (original_key, original_value) = override_event
            .recurrence_id
            .as_ref()
            .ok_or("Missing RECURRENCE-ID")?;
        // Strict source-property identity: forbid TZID aliases, floating/UTC
        // reinterpretations, RANGE=THISANDFUTURE, and mixed DATE/DATE-TIME.
        let suffix = source
            .dtstart
            .0
            .strip_prefix("DTSTART")
            .ok_or("Invalid source DTSTART")?;
        let expected_key = format!("RECURRENCE-ID{suffix}");
        if original_key != &expected_key
            || source.dtstart.1.ends_with('Z') != original_value.ends_with('Z')
        {
            return Err(
                "RECURRENCE-ID timezone/kind differs from the master or has unsupported RANGE"
                    .into(),
            );
        }
        let all_day = source.dtstart.1.len() == 8;
        let original_values = occurrence_date_values(
            &[(original_key.clone(), original_value.clone())],
            all_day,
            floating_timezone,
        )?;
        let [original_ms] = original_values.as_slice() else {
            return Err("RECURRENCE-ID must identify exactly one original occurrence".into());
        };
        let target_id = format!("{series_key}:{original_ms}");
        if !modified.insert(target_id.clone()) {
            return Err("Duplicate RECURRENCE-ID for one original occurrence".into());
        }
        let index = events
            .iter()
            .position(|event| {
                event.id == target_id && event.series_uid.as_deref() == Some(&series_key)
            })
            .ok_or("RECURRENCE-ID does not match a materialized original occurrence")?;
        if override_event.status.as_deref() == Some("CANCELLED") {
            if override_event.has_alarm
                || override_event.end_property.is_some()
                || override_event.duration_property.is_some()
                || override_event
                    .start_property
                    .as_ref()
                    .is_some_and(|(key, value)| key != &source.dtstart.0 || value != original_value)
            {
                return Err("Cancellation must not change dates, duration, or reminders".into());
            }
            source.cancellations.push(SourceOccurrenceCancellation {
                recurrence_id: (original_key.clone(), original_value.clone()),
                occurrence_id: target_id,
            });
            events.remove(index);
            if !source.valid(&series_key) {
                return Err("Cancellation exceeds safe persisted source metadata limits".into());
            }
            continue;
        }
        let title = override_event
            .title
            .ok_or("Override requires explicit SUMMARY")?;
        if override_event.start_property.as_ref().map(|item| &item.0) != Some(&source.dtstart.0)
            || override_event
                .start_property
                .as_ref()
                .is_none_or(|item| item.1.ends_with('Z') != source.dtstart.1.ends_with('Z'))
        {
            return Err("Override DTSTART must keep the master's exact timezone and kind".into());
        }
        if override_event.end_property.is_some() && override_event.duration_property.is_some() {
            return Err("Override cannot combine DTEND and DURATION".into());
        }
        // Explicit end is mandatory if the source had one; otherwise an omitted
        // end could unknowingly change recurrence duration.
        if source.dtend.is_some() && override_event.end_property.is_none() {
            return Err("Override must explicitly include inherited DTEND".into());
        }
        if source.duration.is_some() && override_event.duration_property.is_none() {
            return Err("Override must explicitly include inherited DURATION".into());
        }
        if source.reminder_minutes.is_some() && override_event.reminder.is_none() {
            return Err(
                "Override must explicitly retain its source reminder or fail safely".into(),
            );
        }
        let (start_ms, end_ms, span) = if all_day {
            let day = override_event
                .all_day_start
                .ok_or("All-day override requires date-only DTSTART")?;
            if override_event.start.is_some()
                || override_event.duration_property.is_some()
                || override_event.reminder.is_some()
            {
                return Err("All-day override cannot contain timed fields or alarms".into());
            }
            let next = if let Some((key, value)) = override_event.end_property.as_ref() {
                if source
                    .dtend
                    .as_ref()
                    .is_some_and(|existing| existing.0 != *key)
                {
                    return Err("All-day override DTEND kind must match its source".into());
                }
                parse_ics_date(key, value)?
            } else {
                day.succ_opt().ok_or("All-day override range overflow")?
            };
            let span = AllDaySpan::new(day, next)?;
            (span.utc_midnight_ms()?, None, Some(span))
        } else {
            if override_event.all_day_start.is_some() {
                return Err("Timed override cannot become all-day".into());
            }
            let start = override_event
                .start
                .as_ref()
                .ok_or("Timed override requires DTSTART")?
                .occurrences(None)?[0];
            let elapsed = if let Some((key, value)) = override_event.end_property.as_ref() {
                if source
                    .dtend
                    .as_ref()
                    .is_some_and(|existing| existing.0 != *key)
                {
                    return Err("Override DTEND timezone/kind must match its source".into());
                }
                let end =
                    CalendarStart::parse(key, value, floating_timezone)?.occurrences(None)?[0];
                Some(
                    end.checked_sub(start)
                        .filter(|span| *span > 0)
                        .ok_or("Override DTEND must be after DTSTART")?,
                )
            } else {
                override_event
                    .duration_property
                    .as_deref()
                    .map(parse_timed_duration)
                    .transpose()?
            };
            let end = elapsed
                .map(|span| start.checked_add(span).ok_or("Override end overflow"))
                .transpose()?;
            validate_timed_end(start, end)?;
            (start, end, None)
        };
        if events.iter().enumerate().any(|(i, event)| {
            i != index
                && event.series_uid.as_deref() == Some(&series_key)
                && event.starts_at_ms == start_ms
        }) {
            return Err("Override would collide with another occurrence in the series".into());
        }
        validate_event(
            &title,
            &override_event.description,
            start_ms,
            override_event.reminder,
        )?;
        events[index].starts_at_ms = start_ms;
        events[index].ends_at_ms = end_ms;
        events[index].all_day = span;
        events[index].title = title.clone();
        events[index].description = override_event.description.clone();
        events[index].remind_minutes = override_event.reminder;
        // No source metadata is fabricated from flattened events; an imported
        // override retains all explicit fields for a strict reimport round trip.
        source.overrides.push(SourceOccurrenceOverride {
            recurrence_id: (original_key.clone(), original_value.clone()),
            dtstart: override_event
                .start_property
                .ok_or("Missing override source DTSTART")?,
            dtend: override_event.end_property,
            duration: override_event.duration_property,
            title,
            description: override_event.description,
            reminder_minutes: override_event.reminder,
        });
        if !source.valid(&series_key) {
            return Err("Override exceeds safe persisted source metadata limits".into());
        }
    }
    Ok(())
}

fn parse_ics_bundle(
    source: &str,
    floating_timezone: Option<&str>,
) -> Result<(Vec<CalendarEvent>, Vec<SeriesSource>), String> {
    if source.len() > MAX_ICS_BYTES {
        return Err("ICS input exceeds 256 KiB safety limit".into());
    }
    let mut unfolded: Vec<String> = Vec::new();
    for raw in source.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with(' ') || line.starts_with('\t') {
            let parent = unfolded.last_mut().ok_or("Invalid first folded ICS line")?;
            parent.push_str(line.trim_start_matches([' ', '\t']));
        } else {
            unfolded.push(line.to_owned());
        }
    }
    let mut calendar_open = false;
    let mut calendar_end = false;
    let mut in_alarm = false;
    let mut current: Option<PartialEvent> = None;
    let mut events = Vec::new();
    let mut sources = Vec::new();
    let mut pending_overrides = Vec::new();
    let mut method = None;
    for line in unfolded {
        let Some((full_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = full_key
            .split(';')
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let value = raw_value.trim();
        match (key.as_str(), value) {
            ("BEGIN", "VCALENDAR") => {
                if calendar_open {
                    return Err("Nested VCALENDAR".into());
                }
                calendar_open = true;
            }
            ("END", "VCALENDAR") => {
                if current.is_some() {
                    return Err("Unclosed VEVENT".into());
                }
                calendar_end = true;
            }
            ("BEGIN", "VEVENT") if calendar_open && !calendar_end => {
                if current.is_some() {
                    return Err("Nested VEVENT".into());
                }
                current = Some(PartialEvent::default());
            }
            ("METHOD", _) if calendar_open && !calendar_end && current.is_none() => {
                if method.replace(value.to_ascii_uppercase()).is_some() {
                    return Err("Duplicate VCALENDAR METHOD".into());
                }
            }
            ("END", "VEVENT") => {
                let event = current.take().ok_or("VEVENT end without start")?;
                if event.status.as_deref() == Some("CANCELLED") && event.recurrence_id.is_none() {
                    return Err(
                        "Whole-event or whole-series ICS cancellation is unsupported".into(),
                    );
                }
                if event.recurrence_id.is_some() {
                    pending_overrides.push(event);
                    if pending_overrides.len() > MAX_IMPORT_EVENTS {
                        return Err("More than 250 recurrence overrides".into());
                    }
                    continue;
                }
                let title = event.title.ok_or("VEVENT is missing SUMMARY")?;
                let is_all_day = event.all_day_start.is_some();
                if is_all_day && event.start.is_some() {
                    return Err("Mixed date-only and timed DTSTART values".into());
                }
                if !is_all_day && event.start.is_none() {
                    return Err("VEVENT is missing DTSTART".into());
                }
                let base_id = event_id(event.uid.as_deref());
                if event.end_property.is_some() && event.duration_property.is_some() {
                    return Err("VEVENT cannot contain both DTEND and DURATION".into());
                }
                if is_all_day && event.duration_property.is_some() {
                    return Err(
                        "DATE-only DURATION is not supported; use exclusive DATE DTEND".into(),
                    );
                }
                let base_span = if let Some(start) = event.all_day_start {
                    let end = if let Some((key, raw)) = event.end_property.as_ref() {
                        parse_ics_date(key, raw)?
                    } else {
                        start.succ_opt().ok_or("All-day end date overflow")?
                    };
                    if event.reminder.is_some() {
                        return Err(
                            "All-day VALARM is unsupported; cannot schedule at UTC midnight".into(),
                        );
                    }
                    Some(AllDaySpan::new(start, end)?)
                } else {
                    None
                };
                let elapsed_ms = if !is_all_day {
                    let original_start = event
                        .start
                        .as_ref()
                        .ok_or("Missing DTSTART")?
                        .occurrences(None)?[0];
                    let elapsed = if let Some((key, raw)) = event.end_property.as_ref() {
                        let end = CalendarStart::parse(key, raw, floating_timezone)?
                            .occurrences(None)?[0];
                        Some(
                            end.checked_sub(original_start)
                                .filter(|duration| *duration > 0)
                                .ok_or("Timed DTEND must be strictly later than DTSTART")?,
                        )
                    } else {
                        event
                            .duration_property
                            .as_ref()
                            .map(|raw| parse_timed_duration(raw))
                            .transpose()?
                    };
                    if let Some(ms) = elapsed {
                        let end = original_start
                            .checked_add(ms)
                            .ok_or("Timed end timestamp overflow")?;
                        validate_timed_end(original_start, Some(end))?;
                    }
                    elapsed
                } else {
                    None
                };
                let mut instants = if let Some(span) = base_span.as_ref() {
                    let date = span.start_date.replace('-', "");
                    let source = CalendarStart::parse("DTSTART", &format!("{date}T000000Z"), None)?;
                    source.occurrences(event.recurrence.as_deref())?
                } else {
                    event
                        .start
                        .as_ref()
                        .ok_or("Missing DTSTART")?
                        .occurrences(event.recurrence.as_deref())?
                };
                let exclusions =
                    occurrence_date_values(&event.exdates, is_all_day, floating_timezone)?;
                let additions =
                    occurrence_date_values(&event.rdates, is_all_day, floating_timezone)?;
                instants.extend(additions);
                let excluded: HashSet<_> = exclusions.into_iter().collect();
                instants.retain(|instant| !excluded.contains(instant));
                instants.sort_unstable();
                instants.dedup();
                if instants.is_empty() || instants.len() + events.len() > MAX_IMPORT_EVENTS {
                    return Err("ICS event has no remaining occurrences or exceeds the 250-event import limit".into());
                }
                let is_series = event.recurrence.is_some() || !event.rdates.is_empty();
                if is_series
                    && !event
                        .uid
                        .as_deref()
                        .is_some_and(|uid| valid_uid(uid) && uid.len() <= 128)
                {
                    return Err(
                        "Recurring imports require a stable UID of at most 128 bytes".into(),
                    );
                }
                let series_uid = is_series.then_some(base_id.clone());
                if is_series {
                    let source_id = base_id.clone();
                    if sources
                        .iter()
                        .any(|item: &SeriesSource| item.uid == source_id)
                    {
                        return Err("Duplicate recurring source UID; no events imported".into());
                    }
                    let source = SeriesSource {
                        schema_version: 1,
                        uid: source_id.clone(),
                        title: title.clone(),
                        description: event.description.clone(),
                        dtstart: event
                            .start_property
                            .clone()
                            .ok_or("Missing source DTSTART")?,
                        dtend: event.end_property.clone(),
                        duration: event.duration_property.clone(),
                        rrule: event.recurrence.clone(),
                        exdates: event.exdates.clone(),
                        rdates: event.rdates.clone(),
                        floating_timezone: floating_timezone.map(str::to_owned),
                        reminder_minutes: event.reminder,
                        modified_occurrence_ids: HashSet::new(),
                        overrides: Vec::new(),
                        cancellations: Vec::new(),
                    };
                    if !source.valid(&source_id) {
                        return Err("ICS series metadata is outside safe persistence bounds".into());
                    }
                    sources.push(source);
                }
                let first_instant = base_span
                    .as_ref()
                    .map(AllDaySpan::utc_midnight_ms)
                    .transpose()?;
                for starts_at_ms in instants {
                    let all_day =
                        if let (Some(span), Some(first)) = (base_span.as_ref(), first_instant) {
                            let current_day = DateTime::<Utc>::from_timestamp_millis(
                                i64::try_from(starts_at_ms).map_err(|_| "Date out of range")?,
                            )
                            .ok_or("Invalid occurrence timestamp")?
                            .date_naive();
                            let original_day = DateTime::<Utc>::from_timestamp_millis(
                                i64::try_from(first).map_err(|_| "Start date out of range")?,
                            )
                            .ok_or("Invalid start date")?
                            .date_naive();
                            Some(span.shift_days(
                                current_day.signed_duration_since(original_day).num_days(),
                            )?)
                        } else {
                            None
                        };
                    validate_event(&title, &event.description, starts_at_ms, event.reminder)?;
                    let ends_at_ms = elapsed_ms
                        .map(|ms| {
                            starts_at_ms
                                .checked_add(ms)
                                .ok_or("Timed end exceeds supported range")
                        })
                        .transpose()?;
                    validate_timed_end(starts_at_ms, ends_at_ms)?;
                    // Keep legacy single-UID identity for nonrecurring imports.
                    // In a series use the absolute occurrence key independent of
                    // EXDATE/order; never silently remap another occurrence's ID.
                    let id = if is_series {
                        format!("{base_id}:{starts_at_ms}")
                    } else {
                        base_id.clone()
                    };
                    events.push(CalendarEvent {
                        id,
                        title: title.clone(),
                        starts_at_ms,
                        remind_minutes: event.reminder,
                        description: event.description.clone(),
                        created_ms: now_ms(),
                        all_day,
                        series_uid: series_uid.clone(),
                        ends_at_ms,
                    });
                }
            }
            ("BEGIN", "VALARM") if current.is_some() => {
                in_alarm = true;
                if let Some(event) = current.as_mut() {
                    event.has_alarm = true;
                }
            }
            ("END", "VALARM") => in_alarm = false,
            _ if in_alarm => {
                if key == "TRIGGER" {
                    if let Some(event) = current.as_mut() {
                        event.reminder = match value {
                            "-PT0M" => Some(0),
                            "-PT5M" => Some(5),
                            "-PT15M" => Some(15),
                            "-PT30M" => Some(30),
                            "-PT60M" | "-PT1H" => Some(60),
                            _ => None,
                        };
                    }
                }
            }
            _ => {
                if let Some(event) = current.as_mut() {
                    match key.as_str() {
                        "SUMMARY" => event.title = Some(unescape_text(value)),
                        "DESCRIPTION" => event.description = unescape_text(value),
                        "UID" => {
                            if !full_key.eq_ignore_ascii_case("UID")
                                || event.uid.replace(value.to_owned()).is_some()
                            {
                                return Err("Duplicate or parameterized VEVENT UID".into());
                            }
                        }
                        "DTSTART" => {
                            if event.start.is_some() || event.all_day_start.is_some() {
                                return Err("Duplicate DTSTART".into());
                            }
                            event.start_property = Some((full_key.to_owned(), value.to_owned()));
                            if full_key.to_ascii_uppercase().contains("VALUE=DATE")
                                || value.len() == 8
                            {
                                event.all_day_start = Some(parse_ics_date(full_key, value)?);
                            } else {
                                event.start =
                                    Some(CalendarStart::parse(full_key, value, floating_timezone)?);
                            }
                        }
                        "DTEND" => {
                            if event.end_property.is_some() {
                                return Err("Duplicate DTEND".into());
                            }
                            event.end_property = Some((full_key.to_owned(), value.to_owned()));
                        }
                        "DURATION" => {
                            if event.duration_property.is_some() || full_key != "DURATION" {
                                return Err("Duplicate or parameterized DURATION".into());
                            }
                            event.duration_property = Some(value.to_owned());
                        }
                        "RRULE" => {
                            if event.recurrence.is_some() {
                                return Err("Duplicate RRULE".into());
                            }
                            event.recurrence = Some(value.to_owned());
                        }
                        "EXDATE" => event.exdates.push((full_key.to_owned(), value.to_owned())),
                        "RDATE" => event.rdates.push((full_key.to_owned(), value.to_owned())),
                        "STATUS" => {
                            if !full_key.eq_ignore_ascii_case("STATUS") || event.status.is_some() {
                                return Err("Duplicate or parameterized VEVENT STATUS".into());
                            }
                            event.status = Some(value.to_ascii_uppercase());
                        }
                        "RECURRENCE-ID" => {
                            if event.recurrence_id.is_some() {
                                return Err("Duplicate RECURRENCE-ID".into());
                            }
                            event.recurrence_id = Some((full_key.to_owned(), value.to_owned()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if !calendar_open || !calendar_end || current.is_some() || events.is_empty() {
        return Err("Expected a complete VCALENDAR containing at least one VEVENT".into());
    }
    if method.as_deref().is_some_and(|value| value != "PUBLISH")
        && pending_overrides
            .iter()
            .any(|event| event.status.as_deref() == Some("CANCELLED"))
    {
        return Err(
            "Scheduling-message cancellations are unsupported; import a calendar snapshot".into(),
        );
    }
    apply_ics_overrides(
        &mut events,
        &mut sources,
        pending_overrides,
        floating_timezone,
    )?;
    Ok((events, sources))
}

#[cfg(test)]
fn parse_ics(source: &str, floating_timezone: Option<&str>) -> Result<Vec<CalendarEvent>, String> {
    Ok(parse_ics_bundle(source, floating_timezone)?.0)
}

fn serialize_ics(events: &[CalendarEvent]) -> Result<String, String> {
    let mut output = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//WFS Notch//Local Calendar 1.0//EN\r\nCALSCALE:GREGORIAN\r\n");
    for event in events {
        validate_event(
            &event.title,
            &event.description,
            event.starts_at_ms,
            event.remind_minutes,
        )?;
        let uid = if event.id.starts_with("ics:") {
            event
                .id
                .strip_prefix("ics:")
                .unwrap_or(&event.id)
                .to_owned()
        } else {
            format!("{}{LOCAL_UID_SUFFIX}", event.id)
        };
        for line in [
            "BEGIN:VEVENT".to_owned(),
            format!("UID:{}", escape_text(&uid)),
            format!("DTSTAMP:{}", ms_to_utc(now_ms() as u64)),
        ] {
            output.push_str(&folded_line(&line));
        }
        if let Some(span) = event.all_day.as_ref() {
            if !span.valid()
                || span.utc_midnight_ms()? != event.starts_at_ms
                || event.remind_minutes.is_some()
                || event.ends_at_ms.is_some()
            {
                return Err("Invalid all-day calendar span in persisted data".into());
            }
            output.push_str(&folded_line(&format!(
                "DTSTART;VALUE=DATE:{}",
                AllDaySpan::ics_date(&span.start_date)?
            )));
            output.push_str(&folded_line(&format!(
                "DTEND;VALUE=DATE:{}",
                AllDaySpan::ics_date(&span.end_date_exclusive)?
            )));
        } else {
            validate_timed_end(event.starts_at_ms, event.ends_at_ms)?;
            output.push_str(&folded_line(&format!(
                "DTSTART:{}",
                ms_to_utc(event.starts_at_ms)
            )));
            if let Some(end) = event.ends_at_ms {
                output.push_str(&folded_line(&format!("DTEND:{}", ms_to_utc(end))));
            }
        }
        for line in [
            format!("SUMMARY:{}", escape_text(&event.title)),
            format!("DESCRIPTION:{}", escape_text(&event.description)),
        ] {
            output.push_str(&folded_line(&line));
        }
        if let Some(minutes) = event.remind_minutes {
            output.push_str("BEGIN:VALARM\r\nACTION:DISPLAY\r\n");
            output.push_str(&format!("TRIGGER:-PT{minutes}M\r\n"));
            output.push_str("DESCRIPTION:WFS Notch reminder\r\nEND:VALARM\r\n");
        }
        output.push_str("END:VEVENT\r\n");
    }
    output.push_str("END:VCALENDAR\r\n");
    Ok(output)
}
// Reconstruct an original source master only from versioned, validated
// metadata. All values are folded, and unsafe control bytes are rejected.
// No source file is read and no unsupported RRULE is silently accepted.
fn safe_source_property(key: &str, value: &str) -> Result<String, String> {
    if key.is_empty()
        || key.len() > 110
        || key.contains(':')
        || key.chars().any(char::is_control)
        || value.len() > 512
        || value.chars().any(char::is_control)
    {
        return Err("Unsafe original ICS source property; export refused".into());
    }
    Ok(folded_line(&format!("{key}:{value}")))
}

fn source_override_vevent(
    uid: &str,
    exception: &SourceOccurrenceOverride,
) -> Result<String, String> {
    let mut out = String::from("BEGIN:VEVENT\r\n");
    out.push_str(&safe_source_property("UID", uid)?);
    out.push_str(&safe_source_property(
        "DTSTAMP",
        &ms_to_utc(now_ms() as u64),
    )?);
    out.push_str(&safe_source_property(
        &exception.recurrence_id.0,
        &exception.recurrence_id.1,
    )?);
    out.push_str(&safe_source_property(
        &exception.dtstart.0,
        &exception.dtstart.1,
    )?);
    if let Some((key, value)) = &exception.dtend {
        out.push_str(&safe_source_property(key, value)?);
    }
    if let Some(duration) = &exception.duration {
        out.push_str(&safe_source_property("DURATION", duration)?);
    }
    out.push_str(&folded_line(&format!(
        "SUMMARY:{}",
        escape_text(&exception.title)
    )));
    out.push_str(&folded_line(&format!(
        "DESCRIPTION:{}",
        escape_text(&exception.description)
    )));
    if let Some(minutes) = exception.reminder_minutes {
        out.push_str("BEGIN:VALARM\r\nACTION:DISPLAY\r\n");
        out.push_str(&format!("TRIGGER:-PT{minutes}M\r\n"));
        out.push_str("DESCRIPTION:WFS Notch reminder\r\nEND:VALARM\r\n");
    }
    out.push_str("END:VEVENT\r\n");
    Ok(out)
}

fn source_master_vevent(source: &SeriesSource) -> Result<String, String> {
    if !source.valid(&source.uid) {
        return Err("Invalid persisted calendar source metadata".into());
    }
    let uid = source
        .uid
        .strip_prefix("ics:")
        .ok_or("Invalid ICS source UID")?;
    if !valid_uid(uid) || uid.len() > 128 {
        return Err("Invalid ICS master UID for export".into());
    }
    let mut out = String::from("BEGIN:VEVENT\r\n");
    out.push_str(&safe_source_property("UID", uid)?);
    out.push_str(&safe_source_property(
        "DTSTAMP",
        &ms_to_utc(now_ms() as u64),
    )?);
    out.push_str(&safe_source_property(&source.dtstart.0, &source.dtstart.1)?);
    if let Some((key, value)) = &source.dtend {
        out.push_str(&safe_source_property(key, value)?);
    }
    if let Some(duration) = &source.duration {
        out.push_str(&safe_source_property("DURATION", duration)?);
    }
    if let Some(rule) = &source.rrule {
        out.push_str(&safe_source_property("RRULE", rule)?);
    }
    for (key, value) in &source.exdates {
        out.push_str(&safe_source_property(key, value)?);
    }
    for (key, value) in &source.rdates {
        out.push_str(&safe_source_property(key, value)?);
    }
    out.push_str(&folded_line(&format!(
        "SUMMARY:{}",
        escape_text(&source.title)
    )));
    out.push_str(&folded_line(&format!(
        "DESCRIPTION:{}",
        escape_text(&source.description)
    )));
    if let Some(minutes) = source.reminder_minutes {
        out.push_str("BEGIN:VALARM\r\nACTION:DISPLAY\r\n");
        out.push_str(&format!("TRIGGER:-PT{minutes}M\r\n"));
        out.push_str("DESCRIPTION:WFS Notch reminder\r\nEND:VALARM\r\n");
    }
    out.push_str("END:VEVENT\r\n");
    for exception in &source.overrides {
        out.push_str(&source_override_vevent(uid, exception)?);
    }
    for exception in &source.cancellations {
        out.push_str("BEGIN:VEVENT\r\nSTATUS:CANCELLED\r\n");
        out.push_str(&safe_source_property("UID", uid)?);
        out.push_str(&safe_source_property(
            "DTSTAMP",
            &ms_to_utc(now_ms() as u64),
        )?);
        out.push_str(&safe_source_property(
            &exception.recurrence_id.0,
            &exception.recurrence_id.1,
        )?);
        out.push_str("END:VEVENT\r\n");
    }
    Ok(out)
}

fn source_is_faithful(data: &AppData, source: &SeriesSource) -> Option<String> {
    if !source.valid(&source.uid)
        || !source.modified_occurrence_ids.is_empty()
        || data.calendar_tombstones.contains(&source.uid)
    {
        return None;
    }
    let members: Vec<&CalendarEvent> = data
        .events
        .iter()
        .filter(|event| event.series_uid.as_deref() == Some(&source.uid))
        .collect();
    if members.is_empty() && source.cancellations.is_empty() {
        return None;
    }
    let master = source_master_vevent(source).ok()?;
    let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{master}END:VCALENDAR\r\n");
    // Re-run the very same strict importer to catch unsupported/new metadata.
    let (expected, parsed_sources) =
        parse_ics_bundle(&text, source.floating_timezone.as_deref()).ok()?;
    if parsed_sources.first()?.cancellations != source.cancellations
        || source
            .cancellations
            .iter()
            .any(|exception| !data.calendar_tombstones.contains(&exception.occurrence_id))
    {
        return None;
    }
    if members.len() != expected.len() {
        return None;
    }
    let mut expected_by_id = std::collections::HashMap::new();
    for event in &expected {
        if data.calendar_tombstones.contains(&event.id)
            || expected_by_id.insert(event.id.as_str(), event).is_some()
        {
            return None;
        }
    }
    for actual in members {
        let candidate = expected_by_id.remove(actual.id.as_str())?;
        if actual.title != candidate.title
            || actual.description != candidate.description
            || actual.starts_at_ms != candidate.starts_at_ms
            || actual.ends_at_ms != candidate.ends_at_ms
            || actual.all_day != candidate.all_day
            || actual.remind_minutes != candidate.remind_minutes
            || actual.series_uid != candidate.series_uid
        {
            return None;
        }
    }
    expected_by_id.is_empty().then_some(master)
}

/// Export original VEVENT masters only when the current local series exactly
/// matches the source after strict re-expansion. Any edited/deleted/legacy
/// series is flattened instead, never exported both ways or resurrected.
fn serialize_ics_with_sources(data: &AppData) -> Result<String, String> {
    let mut preserved = HashSet::new();
    let mut masters = String::new();
    let mut keys = data.calendar_series.keys().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        let source = &data.calendar_series[key];
        if let Some(master) = source_is_faithful(data, source) {
            preserved.insert(key.as_str());
            masters.push_str(&master);
        }
    }
    let remaining = data
        .events
        .iter()
        .filter(|event| {
            !event
                .series_uid
                .as_deref()
                .is_some_and(|uid| preserved.contains(uid))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut output = serialize_ics(&remaining)?;
    let end = "END:VCALENDAR\r\n";
    if !output.ends_with(end) {
        return Err("Invalid generated calendar export".into());
    }
    output.truncate(output.len() - end.len());
    output.push_str(&masters);
    output.push_str(end);
    Ok(output)
}

#[tauri::command]
pub fn export_ics(store: State<'_, Mutex<Store>>) -> Result<String, String> {
    let state = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    serialize_ics_with_sources(&state.data)
}
fn merge_imported_events(data: &mut AppData, incoming: Vec<CalendarEvent>) -> Result<(), String> {
    // A Phase 14 series used its root UID for the first occurrence and
    // :timestamp for later ones. Upgrading those records MUST NOT duplicate
    // the first instance, or lose an edited title on reimport.
    let mut known: HashSet<String> = data.events.iter().map(|event| event.id.clone()).collect();
    let mut fresh = Vec::new();
    for candidate in incoming {
        // Tombstones remain local, persist in backups and prevent accidental
        // resurrection of deleted imported occurrences / whole series.
        if data.calendar_tombstones.contains(&candidate.id)
            || candidate
                .series_uid
                .as_ref()
                .is_some_and(|series| data.calendar_tombstones.contains(series))
        {
            continue;
        }
        if let Some(series) = candidate.series_uid.as_ref() {
            if let Some(existing) = data.events.iter_mut().find(|entry| {
                entry.starts_at_ms == candidate.starts_at_ms
                    && (entry.id == *series
                        || entry.id == candidate.id
                        || entry.series_uid.as_ref() == Some(series))
            }) {
                if existing.series_uid.is_none() {
                    existing.series_uid = Some(series.clone());
                }
                continue;
            }
        }
        if known.insert(candidate.id.clone()) {
            fresh.push(candidate);
        }
    }
    if data.events.len() + fresh.len() > 1000 {
        return Err("Event limit (1000) exceeded".into());
    }
    data.events.extend(fresh);
    Ok(())
}

#[derive(Debug, Serialize)]
struct IcsCancellationPreview {
    title: String,
    original: String,
    original_kind: String,
    floating_timezone: Option<String>,
    removes_local_event: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct IcsImportPreview {
    new_events: usize,
    removed_events: usize,
    cancellations: Vec<IcsCancellationPreview>,
    fingerprint: String,
}

fn same_recurrence(a: &SeriesSource, b: &SeriesSource) -> bool {
    a.dtstart == b.dtstart
        && a.rrule == b.rrule
        && a.exdates == b.exdates
        && a.rdates == b.rdates
        && a.floating_timezone == b.floating_timezone
}

pub(super) fn reset_cancellation_history(data: &mut AppData) {
    data.calendar_cancellation_epoch = Some(new_id());
    data.calendar_tombstones.clear();
    for source in data.calendar_series.values_mut() {
        source.cancellations.clear();
    }
}

/// Keep local cancellation decisions across ordinary snapshot restores. If
/// source schedules differ, preserve the marker, not an unprovable exception.
pub(super) fn preserve_cancellation_history(
    restored: &mut AppData,
    current: &AppData,
) -> Result<(), String> {
    let stale_history = current.calendar_cancellation_epoch.is_some()
        && current.calendar_cancellation_epoch != restored.calendar_cancellation_epoch;
    if stale_history {
        restored.calendar_tombstones.clear();
        for source in restored.calendar_series.values_mut() {
            source.cancellations.clear();
        }
    }
    if let Some(epoch) = &current.calendar_cancellation_epoch {
        restored.calendar_cancellation_epoch = Some(epoch.clone());
    }
    for key in &current.calendar_tombstones {
        remember_calendar_cancellation(restored, key)?;
    }
    let markers = &restored.calendar_tombstones;
    let series = &restored.calendar_series;
    restored.events.retain(|event| {
        // Legacy first occurrences used the master UID instead of UID:ms.
        let legacy_series = event.series_uid.as_deref() == Some(event.id.as_str())
            || series.contains_key(&event.id)
            || current.calendar_series.contains_key(&event.id);
        !markers.contains(&event.id)
            && !event
                .series_uid
                .as_ref()
                .is_some_and(|uid| markers.contains(uid))
            && !(legacy_series && markers.contains(&format!("{}:{}", event.id, event.starts_at_ms)))
    });
    restored.calendar_series.retain(|uid, source| {
        if markers.contains(uid) {
            return false;
        }
        source
            .cancellations
            .retain(|entry| markers.contains(&entry.occurrence_id));
        if let Some(live) = current
            .calendar_series
            .get(uid)
            .filter(|live| same_recurrence(source, live))
        {
            for entry in &live.cancellations {
                if !markers.contains(&entry.occurrence_id) || source.cancellations.contains(entry) {
                    continue;
                }
                let mut candidate = source.clone();
                candidate
                    .overrides
                    .retain(|item| item.recurrence_id != entry.recurrence_id);
                candidate.cancellations.push(entry.clone());
                if candidate.valid(uid) {
                    *source = candidate;
                }
            }
        }
        true
    });
    Ok(())
}

fn prepare_import(
    data: &AppData,
    text: &str,
    timezone: Option<&str>,
) -> Result<(IcsImportPreview, AppData), String> {
    let (incoming, sources) = parse_ics_bundle(text, timezone)?;
    let mut candidate = data.clone();
    let mut cancellations = Vec::new();
    let mut removed_events = 0;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    timezone.hash(&mut hasher);
    data.calendar_cancellation_epoch.hash(&mut hasher);
    serde_json::to_vec(&data.events)
        .map_err(|e| e.to_string())?
        .hash(&mut hasher);
    let mut tombstones = data.calendar_tombstones.iter().collect::<Vec<_>>();
    tombstones.sort_unstable();
    tombstones.hash(&mut hasher);
    for source in sources {
        if candidate.calendar_tombstones.contains(&source.uid) {
            continue;
        }
        if let Some(existing) = candidate.calendar_series.get(&source.uid) {
            if !source.cancellations.is_empty() && !same_recurrence(existing, &source) {
                return Err("Cancellation source differs from the saved series schedule".into());
            }
            if !source.cancellations.is_empty() {
                let mut modified = existing.modified_occurrence_ids.iter().collect::<Vec<_>>();
                modified.sort_unstable();
                modified.hash(&mut hasher);
                serde_json::to_vec(&existing.cancellations)
                    .map_err(|e| e.to_string())?
                    .hash(&mut hasher);
            }
        } else if candidate.calendar_series.len() >= 250 {
            return Err("Stored ICS series limit (250) exceeded".into());
        }
        for exception in &source.cancellations {
            let id = &exception.occurrence_id;
            let mut matching = candidate.events.iter().filter(|event| event.id == *id);
            let local = matching.next();
            if incoming
                .iter()
                .any(|event| event.id == *id || event.id == source.uid)
                || matching.next().is_some()
                || local.is_some_and(|event| event.series_uid.as_deref() != Some(&source.uid))
                || candidate.events.iter().any(|event| event.id == source.uid)
                || (local.is_some()
                    && candidate
                        .calendar_series
                        .get(&source.uid)
                        .is_none_or(|saved| saved.modified_occurrence_ids.contains(id)))
            {
                return Err(
                    "Cancellation conflicts with a locally edited, legacy, or unrelated event"
                        .into(),
                );
            }
            let removes_local_event = local.is_some();
            let title = local.map_or_else(|| source.title.clone(), |event| event.title.clone());
            removed_events += usize::from(removes_local_event);
            cancellations.push(IcsCancellationPreview {
                title,
                original: exception.recurrence_id.1.clone(),
                original_kind: exception.recurrence_id.0.clone(),
                floating_timezone: source.floating_timezone.clone(),
                removes_local_event,
            });
            remember_calendar_cancellation(&mut candidate, id)?;
            candidate.events.retain(|event| event.id != *id);
        }
        if let Some(existing) = candidate.calendar_series.get_mut(&source.uid) {
            // Never adopt unrelated incoming edits to an existing series.
            for cancellation in source.cancellations {
                existing
                    .overrides
                    .retain(|entry| entry.recurrence_id != cancellation.recurrence_id);
                if !existing.cancellations.contains(&cancellation) {
                    existing.cancellations.push(cancellation);
                }
            }
            if !existing.valid(&existing.uid) {
                return Err("Cancellation exceeds safe saved series bounds".into());
            }
        } else {
            candidate.calendar_series.insert(source.uid.clone(), source);
        }
    }
    let before = candidate.events.len();
    merge_imported_events(&mut candidate, incoming)?;
    let new_events = candidate.events.len() - before;
    new_events.hash(&mut hasher);
    removed_events.hash(&mut hasher);
    Ok((
        IcsImportPreview {
            new_events,
            removed_events,
            cancellations,
            fingerprint: format!("{:016x}", hasher.finish()),
        },
        candidate,
    ))
}

#[tauri::command]
pub fn preview_ics_import(
    store: State<'_, Mutex<Store>>,
    ics_text: String,
    floating_timezone: Option<String>,
) -> Result<IcsImportPreview, String> {
    let snapshot = store
        .lock()
        .map_err(|_| "Storage lock poisoned")?
        .data
        .clone();
    Ok(prepare_import(&snapshot, &ics_text, floating_timezone.as_deref())?.0)
}

fn import_after_preview(
    data: &mut AppData,
    text: &str,
    timezone: Option<&str>,
    fingerprint: Option<&str>,
) -> Result<(), String> {
    let (preview, candidate) = prepare_import(data, text, timezone)?;
    if (fingerprint.is_some() || !preview.cancellations.is_empty())
        && fingerprint != Some(preview.fingerprint.as_str())
    {
        return Err(
            "ICS import preview is missing or stale. Preview the current changes again.".into(),
        );
    }
    *data = candidate;
    Ok(())
}

#[tauri::command]
pub fn import_ics(
    store: State<'_, Mutex<Store>>,
    ics_text: String,
    floating_timezone: Option<String>,
    expected_fingerprint: Option<String>,
) -> Result<AppData, String> {
    with_store(store, move |data| {
        import_after_preview(
            data,
            &ics_text,
            floating_timezone.as_deref(),
            expected_fingerprint.as_deref(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_round_trip_and_leap_year() {
        for value in ["20000229T000000Z", "20261107T101203Z", "20991231T235959Z"] {
            assert_eq!(ms_to_utc(utc_to_ms(value).unwrap()), value);
        }
        for value in [
            "20230229T000000Z",
            "20261301T000000Z",
            "20240101T250000Z",
            "20270101T120000",
        ] {
            assert!(utc_to_ms(value).is_err());
        }
    }
    #[test]
    fn escaping_and_folding() {
        let s = "Line one,\nsemi; slash\\";
        assert_eq!(unescape_text(&escape_text(s)), s);
        let folded = folded_line(&format!("SUMMARY:{}", "é".repeat(50)));
        assert!(folded.contains("\r\n "));
    }
    #[test]
    fn import_export_and_dedupe_identifiers() {
        let event = CalendarEvent {
            id: "original-id".into(),
            title: "Hello,\nworld".into(),
            starts_at_ms: utc_to_ms("20270614T043000Z").unwrap(),
            remind_minutes: Some(15),
            description: "Notes; second line".into(),
            created_ms: 0,
            all_day: None,
            series_uid: None,
            ends_at_ms: None,
        };
        let serialized = serialize_ics(&[event]).unwrap();
        let parsed = parse_ics(&serialized, None).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "original-id");
        assert_eq!(parsed[0].title, "Hello,\nworld");
        assert_eq!(parsed[0].remind_minutes, Some(15));
    }
    #[test]
    fn timezone_import_and_reimport_are_deterministic() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:meeting-2027@example.org\nSUMMARY:Jakarta meeting\nDTSTART;TZID=Asia/Jakarta:20270614T113000\nEND:VEVENT\nEND:VCALENDAR";
        let first = parse_ics(ics, None).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].starts_at_ms,
            utc_to_ms("20270614T043000Z").unwrap()
        );
        let second = parse_ics(ics, None).unwrap();
        assert_eq!(first[0].id, second[0].id);
        // No new JSON state schema: exported occurrences remain UTC.
        let exported = serialize_ics(&first).unwrap();
        let again = parse_ics(&exported, None).unwrap();
        assert_eq!(again[0].id, first[0].id);
        assert_eq!(again[0].starts_at_ms, first[0].starts_at_ms);
    }

    #[test]
    fn floating_import_requires_selected_timezone() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:float@example.org\nSUMMARY:Local appointment\nDTSTART:20270614T113000\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(ics, None).unwrap_err().contains("Floating"));
        let imported = parse_ics(ics, Some("Asia/Jakarta")).unwrap();
        assert_eq!(
            imported[0].starts_at_ms,
            utc_to_ms("20270614T043000Z").unwrap()
        );
    }

    #[test]
    fn daily_recurrence_expands_without_shift_or_duplicate_ids() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:series@example.org\nSUMMARY:Recurring event\nDTSTART;TZID=America/New_York:20270313T093000\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let one = parse_ics(ics, None).unwrap();
        let two = parse_ics(ics, None).unwrap();
        assert_eq!(one.len(), 3);
        assert_eq!(
            one.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            two.iter().map(|e| e.id.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(one[0].starts_at_ms, utc_to_ms("20270313T143000Z").unwrap());
        assert_eq!(one[1].starts_at_ms, utc_to_ms("20270314T133000Z").unwrap());
        assert_eq!(one[2].starts_at_ms, utc_to_ms("20270315T133000Z").unwrap());
        let exported = serialize_ics(&one).unwrap();
        assert_eq!(parse_ics(&exported, None).unwrap().len(), 3);
        assert!(!exported.contains("RRULE:")); // intentionally flattened
    }

    #[test]
    fn recurrence_id_relocates_one_occurrence_with_stable_id_and_strict_roundtrip() {
        let input = concat!(
            "BEGIN:VCALENDAR\nVERSION:2.0\n",
            "BEGIN:VEVENT\nUID:scope2027@example.org\n",
            "SUMMARY:Master\nDESCRIPTION:Original description\n",
            "DTSTART:20270614T090000Z\nDTEND:20270614T100000Z\n",
            "RRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n",
            "BEGIN:VEVENT\nUID:scope2027@example.org\n",
            "RECURRENCE-ID:20270615T090000Z\n",
            "SUMMARY:Moved only second\nDESCRIPTION:Exception notes\n",
            "DTSTART:20270615T113000Z\nDTEND:20270615T123000Z\n",
            "END:VEVENT\nEND:VCALENDAR\n"
        );
        let (events, sources) = parse_ics_bundle(input, None).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].overrides.len(), 1);
        assert_eq!(events.len(), 3);
        let original_second = utc_to_ms("20270615T090000Z").unwrap();
        let moved_second = events
            .iter()
            .find(|item| item.id == format!("ics:scope2027@example.org:{original_second}"))
            .unwrap();
        assert_eq!(moved_second.title, "Moved only second");
        assert_eq!(moved_second.description, "Exception notes");
        assert_eq!(
            moved_second.starts_at_ms,
            utc_to_ms("20270615T113000Z").unwrap()
        );
        assert_eq!(
            moved_second.ends_at_ms,
            Some(utc_to_ms("20270615T123000Z").unwrap())
        );
        assert_eq!(
            events.iter().filter(|item| item.title == "Master").count(),
            2
        );
        let mut data = AppData {
            events: events.clone(),
            ..Default::default()
        };
        data.calendar_series
            .insert(sources[0].uid.clone(), sources[0].clone());
        let output = serialize_ics_with_sources(&data).unwrap();
        assert_eq!(output.matches("RRULE:").count(), 1);
        assert_eq!(output.matches("RECURRENCE-ID:").count(), 1);
        let (reimported, reimported_sources) = parse_ics_bundle(&output, None).unwrap();
        let canonical = |mut list: Vec<CalendarEvent>| {
            list.sort_by(|a, b| a.id.cmp(&b.id));
            list.into_iter()
                .map(|item| {
                    (
                        item.id,
                        item.title,
                        item.description,
                        item.starts_at_ms,
                        item.ends_at_ms,
                        item.series_uid,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(canonical(events), canonical(reimported.clone()));
        assert_eq!(reimported_sources.len(), 1);
        merge_imported_events(&mut data, reimported).unwrap();
        assert_eq!(data.events.len(), 3);
        // A subsequent local title edit must flatten the entire source
        // instead of exporting a stale RRULE plus changed instance.
        data.events[1].title = "Local user edit".into();
        let flattened = serialize_ics_with_sources(&data).unwrap();
        assert!(!flattened.contains("RRULE:"));
        assert!(!flattened.contains("RECURRENCE-ID:"));
        assert_eq!(parse_ics(&flattened, None).unwrap().len(), 3);
    }

    #[test]
    fn occurrence_override_preserves_iana_dst_instant_and_date_only_span() {
        let local = concat!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:tz-override@example.org\n",
            "SUMMARY:Local master\nDTSTART;TZID=America/New_York:20270313T093000\n",
            "DURATION:PT1H\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n",
            "BEGIN:VEVENT\nUID:tz-override@example.org\n",
            "RECURRENCE-ID;TZID=America/New_York:20270314T093000\n",
            "SUMMARY:Local moved\nDTSTART;TZID=America/New_York:20270314T103000\n",
            "DURATION:PT1H\nEND:VEVENT\nEND:VCALENDAR"
        );
        let (events, source) = parse_ics_bundle(local, None).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(source[0].overrides.len(), 1);
        let second = events
            .iter()
            .find(|item| {
                item.id
                    .ends_with(&utc_to_ms("20270314T133000Z").unwrap().to_string())
            })
            .unwrap();
        assert_eq!(second.starts_at_ms, utc_to_ms("20270314T143000Z").unwrap());
        assert_eq!(
            second.ends_at_ms,
            Some(utc_to_ms("20270314T153000Z").unwrap())
        );
        let mut data = AppData {
            events,
            ..Default::default()
        };
        data.calendar_series
            .insert(source[0].uid.clone(), source[0].clone());
        assert!(serialize_ics_with_sources(&data)
            .unwrap()
            .contains("RECURRENCE-ID;TZID=America/New_York:20270314T093000"));

        let days = concat!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:days2027@example.org\n",
            "SUMMARY:All-day source\nDTSTART;VALUE=DATE:20270614\n",
            "DTEND;VALUE=DATE:20270615\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n",
            "BEGIN:VEVENT\nUID:days2027@example.org\n",
            "RECURRENCE-ID;VALUE=DATE:20270615\n",
            "SUMMARY:Moved date\nDTSTART;VALUE=DATE:20270618\n",
            "DTEND;VALUE=DATE:20270619\nEND:VEVENT\nEND:VCALENDAR"
        );
        let (dates, sources) = parse_ics_bundle(days, None).unwrap();
        assert_eq!(dates.len(), 3);
        let item = dates
            .iter()
            .find(|event| event.title == "Moved date")
            .unwrap();
        assert_eq!(
            item.id,
            format!(
                "ics:days2027@example.org:{}",
                utc_to_ms("20270615T000000Z").unwrap()
            )
        );
        assert_eq!(item.all_day.as_ref().unwrap().start_date, "2027-06-18");
        assert_eq!(
            item.all_day.as_ref().unwrap().end_date_exclusive,
            "2027-06-19"
        );
        let mut data = AppData {
            events: dates,
            ..Default::default()
        };
        data.calendar_series
            .insert(sources[0].uid.clone(), sources[0].clone());
        let export = serialize_ics_with_sources(&data).unwrap();
        assert_eq!(export.matches("RECURRENCE-ID;VALUE=DATE:").count(), 1);
        assert_eq!(parse_ics(&export, None).unwrap().len(), 3);
    }

    #[test]
    fn recurrence_id_ambiguous_unsupported_or_collision_fails_closed() {
        let master = concat!(
            "BEGIN:VEVENT\nUID:safe2027@example.org\n",
            "SUMMARY:Master\nDTSTART:20270614T090000Z\n",
            "RRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n"
        );
        let prefix = "BEGIN:VCALENDAR\n";
        let suffix = "END:VCALENDAR\n";
        let bad = [
            // Mismatch with all original DTSTART instants.
            "UID:safe2027@example.org\nRECURRENCE-ID:20270618T090000Z\nSUMMARY:X\nDTSTART:20270618T090000Z\n",
            // Prohibited RANGE=THISANDFUTURE and TZID reinterpretation.
            "UID:safe2027@example.org\nRECURRENCE-ID;RANGE=THISANDFUTURE:20270615T090000Z\nSUMMARY:X\nDTSTART:20270615T113000Z\n",
            "UID:safe2027@example.org\nRECURRENCE-ID;TZID=Etc/UTC:20270615T090000\nSUMMARY:X\nDTSTART:20270615T113000Z\n",
            // Only one occurrence may resolve to a single start in this series.
            "UID:safe2027@example.org\nRECURRENCE-ID:20270615T090000Z\nSUMMARY:X\nDTSTART:20270616T090000Z\n",
            // No unknown standalone occurrence UID may be silently grafted.
            "UID:other@example.org\nRECURRENCE-ID:20270615T090000Z\nSUMMARY:X\nDTSTART:20270615T113000Z\n",
            // A cancellation cannot also move its occurrence or define an alarm.
            "UID:safe2027@example.org\nRECURRENCE-ID:20270615T090000Z\nSTATUS:CANCELLED\nDTSTART:20270615T113000Z\n",
            "UID:safe2027@example.org\nRECURRENCE-ID:20270615T090000Z\nSTATUS:CANCELLED\nBEGIN:VALARM\nTRIGGER:-PT7M\nEND:VALARM\n",
            "UID:safe2027@example.org\nRECURRENCE-ID:20270615T090000Z,20270616T090000Z\nSTATUS:CANCELLED\n",
            // An override cannot silently change the all-day/timed kind.
            "UID:safe2027@example.org\nRECURRENCE-ID:20270615T090000Z\nSUMMARY:X\nDTSTART;VALUE=DATE:20270615\n",
        ];
        for exception in bad {
            let input = format!("{prefix}{master}BEGIN:VEVENT\n{exception}END:VEVENT\n{suffix}");
            assert!(parse_ics_bundle(&input, None).is_err(), "{exception}");
        }
        let repeated = concat!(
            "BEGIN:VEVENT\nUID:safe2027@example.org\n",
            "RECURRENCE-ID:20270615T090000Z\nSUMMARY:X\nDTSTART:20270615T113000Z\nEND:VEVENT\n"
        );
        let duplicate = format!("{prefix}{master}{repeated}{repeated}{suffix}");
        assert!(parse_ics_bundle(&duplicate, None).is_err());
        let dst = concat!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:gap2027@example.org\n",
            "SUMMARY:NY master\nDTSTART;TZID=America/New_York:20270313T093000\n",
            "RRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n",
            "BEGIN:VEVENT\nUID:gap2027@example.org\n",
            "RECURRENCE-ID;TZID=America/New_York:20270314T093000\n",
            "SUMMARY:DST gap\nDTSTART;TZID=America/New_York:20270314T023000\n",
            "END:VEVENT\nEND:VCALENDAR"
        );
        assert!(parse_ics_bundle(dst, None)
            .unwrap_err()
            .contains("Nonexistent"));
    }

    const CANCELLATION_MASTER: &str = concat!(
        "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:scoped@example.org\n",
        "SUMMARY:Weekly planning\nDTSTART:20270614T090000Z\n",
        "RRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\n"
    );
    const CANCELLATION: &str = concat!(
        "BEGIN:VEVENT\nUID:scoped@example.org\nRECURRENCE-ID:20270615T090000Z\n",
        "STATUS:CANCELLED\nEND:VEVENT\n"
    );

    #[test]
    fn old_backup_keeps_cancellations_and_faithful_export() {
        let plain = format!("{CANCELLATION_MASTER}END:VCALENDAR");
        let canceled = format!("{CANCELLATION_MASTER}{CANCELLATION}END:VCALENDAR");
        let mut old = AppData::default();
        import_after_preview(&mut old, &plain, None, None).unwrap();
        let mut current = old.clone();
        let plan = prepare_import(&current, &canceled, None).unwrap().0;
        import_after_preview(&mut current, &canceled, None, Some(&plan.fingerprint)).unwrap();
        let stale = prepare_import(&old, &plain, None).unwrap().0;
        preserve_cancellation_history(&mut old, &current).unwrap();
        assert_eq!(old.events, current.events);
        assert_eq!(old.calendar_tombstones, current.calendar_tombstones);
        assert!(serialize_ics_with_sources(&old)
            .unwrap()
            .contains("STATUS:CANCELLED"));
        assert!(import_after_preview(&mut old, &plain, None, Some(&stale.fingerprint)).is_err());
        let canceled_backup = old.clone();
        reset_cancellation_history(&mut old);
        let mut restored = canceled_backup;
        preserve_cancellation_history(&mut restored, &old).unwrap();
        assert!(restored.calendar_tombstones.is_empty());
        assert!(!serialize_ics_with_sources(&restored)
            .unwrap()
            .contains("STATUS:CANCELLED"));
        import_after_preview(&mut restored, &plain, None, None).unwrap();
        assert_eq!(restored.events.len(), 3);
    }

    #[test]
    fn restore_filters_legacy_occurrence_and_whole_series_without_copying_mismatched_source() {
        let plain = format!("{CANCELLATION_MASTER}END:VCALENDAR");
        let canceled = format!("{CANCELLATION_MASTER}{CANCELLATION}END:VCALENDAR");
        let mut restored = AppData::default();
        import_after_preview(&mut restored, &plain, None, None).unwrap();
        let mut current = AppData::default();
        let plan = prepare_import(&current, &canceled, None).unwrap().0;
        import_after_preview(&mut current, &canceled, None, Some(&plan.fingerprint)).unwrap();
        let uid = "ics:scoped@example.org";
        restored.calendar_series.get_mut(uid).unwrap().rrule = Some("FREQ=DAILY;COUNT=4".into());
        restored.events[1].id = uid.into();
        preserve_cancellation_history(&mut restored, &current).unwrap();
        assert_eq!(restored.events.len(), 2);
        assert!(restored.calendar_series[uid].cancellations.is_empty());
        remember_calendar_cancellation(&mut current, uid).unwrap();
        preserve_cancellation_history(&mut restored, &current).unwrap();
        assert!(restored.events.is_empty());
        assert!(!restored.calendar_series.contains_key(uid));
    }

    #[test]
    fn scoped_cancellation_previews_commits_and_round_trips_without_resurrection() {
        let plain = format!("{CANCELLATION_MASTER}END:VCALENDAR");
        let canceled = format!("{CANCELLATION_MASTER}{CANCELLATION}END:VCALENDAR");
        let mut data = AppData::default();
        import_after_preview(&mut data, &plain, None, None).unwrap();
        let before = data.clone();
        let id = data.events[1].id.clone();
        let (plan, _) = prepare_import(&data, &canceled, None).unwrap();
        assert_eq!(plan.removed_events, 1);
        assert_eq!(plan.new_events, 0);
        assert_eq!(plan.cancellations[0].original, "20270615T090000Z");
        assert_eq!(data, before);
        assert!(import_after_preview(&mut data, &canceled, None, None).is_err());
        data.events[0].title = "Different local event".into();
        assert!(import_after_preview(&mut data, &canceled, None, Some(&plan.fingerprint)).is_err());
        data = before;
        import_after_preview(&mut data, &canceled, None, Some(&plan.fingerprint)).unwrap();
        assert_eq!(data.events.len(), 2);
        assert!(data.calendar_tombstones.contains(&id));
        let export = serialize_ics_with_sources(&data).unwrap();
        assert_eq!(export.matches("STATUS:CANCELLED").count(), 1);
        assert!(export.contains("RRULE:FREQ=DAILY;COUNT=3"));
        let mut copy = AppData::default();
        let (plan, _) = prepare_import(&copy, &export, None).unwrap();
        import_after_preview(&mut copy, &export, None, Some(&plan.fingerprint)).unwrap();
        assert_eq!(copy.events.len(), 2);
        let restored = serde_json::to_string(&copy).unwrap();
        let mut restored: AppData = serde_json::from_str(&restored).unwrap();
        import_after_preview(&mut restored, &plain, None, None).unwrap();
        assert_eq!(restored.events.len(), 2);
        assert!(restored.calendar_tombstones.contains(&id));
    }

    #[test]
    fn scoped_cancellation_rejects_scheduling_messages_and_local_edits_atomically() {
        let plain = format!("{CANCELLATION_MASTER}END:VCALENDAR");
        let canceled = format!("{CANCELLATION_MASTER}{CANCELLATION}END:VCALENDAR");
        let mut data = AppData::default();
        import_after_preview(&mut data, &plain, None, None).unwrap();
        let target = data.events[1].id.clone();
        data.calendar_series
            .get_mut("ics:scoped@example.org")
            .unwrap()
            .modified_occurrence_ids
            .insert(target);
        let before = data.clone();
        assert!(prepare_import(&data, &canceled, None).is_err());
        assert_eq!(data, before);
        assert!(parse_ics_bundle(&canceled.replace("VERSION:2.0", "METHOD:CANCEL"), None).is_err());
        assert!(parse_ics_bundle(
            &format!("BEGIN:VCALENDAR\n{CANCELLATION}END:VCALENDAR"),
            None
        )
        .is_err());
        assert!(parse_ics_bundle(&canceled.replace("COUNT=3", "COUNT=1"), None).is_err());
        let different = canceled.replace("COUNT=3", "COUNT=4");
        assert!(prepare_import(&data, &different, None)
            .unwrap_err()
            .contains("saved series"));
    }

    #[test]
    fn scoped_cancellation_handles_all_day_and_last_occurrence() {
        let ics = concat!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:all-day@example.org\n",
            "SUMMARY:One day\nDTSTART;VALUE=DATE:20270614\n",
            "RRULE:FREQ=DAILY;COUNT=1\nEND:VEVENT\n",
            "BEGIN:VEVENT\nUID:all-day@example.org\nRECURRENCE-ID;VALUE=DATE:20270614\n",
            "STATUS:CANCELLED\nEND:VEVENT\nEND:VCALENDAR"
        );
        let data = AppData::default();
        let (plan, candidate) = prepare_import(&data, ics, None).unwrap();
        assert_eq!(plan.cancellations.len(), 1);
        assert!(candidate.events.is_empty());
        let export = serialize_ics_with_sources(&candidate).unwrap();
        assert!(export.contains("RECURRENCE-ID;VALUE=DATE:20270614"));
        assert!(parse_ics(&export, None).unwrap().is_empty());
    }

    #[test]
    fn unsupported_rrule_override_and_timed_end_fail_atomically() {
        for suffix in [
            "RRULE:FREQ=MONTHLY;BYSETPOS=1;COUNT=2\n",
            "RRULE:FREQ=DAILY;COUNT=251\n",
            "RRULE:FREQ=DAILY;COUNT=2\nRECURRENCE-ID:20270615T113000Z\n",
            "DTEND:20270614T103000Z\n",
        ] {
            let input = format!(
                "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:series@example.org\nSUMMARY:Series\nDTSTART:20270614T113000Z\n{suffix}END:VEVENT\nEND:VCALENDAR"
            );
            assert!(parse_ics(&input, None).is_err());
        }
    }

    #[test]
    fn timed_dtend_and_duration_round_trip_without_losing_exclusive_utc_end() {
        let input = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:dtend@example.org\nSUMMARY:Timed review\nDTSTART;TZID=Asia/Jakarta:20270614T113000\nDTEND;TZID=Asia/Jakarta:20270614T130000\nEND:VEVENT\nEND:VCALENDAR";
        let first = parse_ics(input, None).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].starts_at_ms,
            utc_to_ms("20270614T043000Z").unwrap()
        );
        assert_eq!(
            first[0].ends_at_ms,
            Some(utc_to_ms("20270614T060000Z").unwrap())
        );
        let text = serialize_ics(&first).unwrap();
        assert!(text.contains("DTEND:20270614T060000Z"));
        let recovered = parse_ics(&text, None).unwrap();
        assert_eq!(recovered[0].starts_at_ms, first[0].starts_at_ms);
        assert_eq!(recovered[0].ends_at_ms, first[0].ends_at_ms);
        assert_eq!(recovered[0].id, first[0].id);
        let duration_ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:duration@example.org\nSUMMARY:Duration\nDTSTART:20270614T113000Z\nDURATION:PT1H30M\nEND:VEVENT\nEND:VCALENDAR";
        let duration = parse_ics(duration_ics, None).unwrap();
        assert_eq!(
            duration[0].ends_at_ms,
            Some(utc_to_ms("20270614T130000Z").unwrap())
        );
        assert!(serialize_ics(&duration)
            .unwrap()
            .contains("DTEND:20270614T130000Z"));
    }

    #[test]
    fn iana_dtend_resolves_spring_dst_and_rejects_nonexistent_and_ambiguous_times() {
        let valid = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:DST change\nDTSTART;TZID=America/New_York:20270313T233000\nDTEND;TZID=America/New_York:20270314T033000\nEND:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(valid, None).unwrap();
        assert_eq!(
            events[0].starts_at_ms,
            utc_to_ms("20270314T043000Z").unwrap()
        );
        assert_eq!(
            events[0].ends_at_ms,
            Some(utc_to_ms("20270314T073000Z").unwrap())
        );
        let gap = valid.replace("20270314T033000", "20270314T023000");
        assert!(parse_ics(&gap, None).unwrap_err().contains("Nonexistent"));
        let overlap = valid.replace("20270314T033000", "20271107T013000");
        assert!(parse_ics(&overlap, None).unwrap_err().contains("Ambiguous"));
        let negative = valid.replace("20270314T033000", "20270313T223000");
        assert!(parse_ics(&negative, None).unwrap_err().contains("later"));
    }

    #[test]
    fn recurring_timed_dtend_preserves_elapsed_duration_across_dst() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:recurring-duration@example.org\nSUMMARY:NY weekly\nDTSTART;TZID=America/New_York:20270313T093000\nDTEND;TZID=America/New_York:20270313T103000\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(ics, None).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0].starts_at_ms,
            utc_to_ms("20270313T143000Z").unwrap()
        );
        assert_eq!(
            events[1].starts_at_ms,
            utc_to_ms("20270314T133000Z").unwrap()
        );
        assert!(events
            .iter()
            .all(|item| item.ends_at_ms.unwrap() - item.starts_at_ms == 3_600_000));
        assert_eq!(
            parse_ics(&serialize_ics(&events).unwrap(), None)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn unsafe_or_unsupported_duration_inputs_fail_closed() {
        for property in [
            "DURATION:P0D",
            "DURATION:-PT1H",
            "DURATION:P999D",
            "DURATION:PT1H1H",
            "DURATION:P1WT1H",
            "DURATION:PT",
            "DURATION:PT0.5H",
            "DTEND:20270614T110000Z",
            "DTEND:20270614T130000Z\nDURATION:PT1H",
        ] {
            let ics = format!("BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Strict\nDTSTART:20270614T113000Z\n{property}\nEND:VEVENT\nEND:VCALENDAR");
            assert!(parse_ics(&ics, None).is_err(), "Should reject {property}");
        }
        let date_with_duration = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:No guessed time\nDTSTART;VALUE=DATE:20270614\nDURATION:P1D\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(date_with_duration, None).is_err());
    }

    #[test]
    fn durable_cancellation_blocks_reimport_until_explicit_reset() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:cancel-me@example.org\nSUMMARY:Local subscription\nDTSTART:20270614T113000Z\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let incoming = parse_ics(ics, None).unwrap();
        let mut state = AppData::default();
        merge_imported_events(&mut state, incoming.clone()).unwrap();
        assert_eq!(state.events.len(), 3);
        crate::remove_series(&mut state, &incoming[0].id).unwrap();
        assert_eq!(state.events.len(), 0);
        assert!(state
            .calendar_tombstones
            .contains("ics:cancel-me@example.org"));
        merge_imported_events(&mut state, incoming.clone()).unwrap();
        assert!(state.events.is_empty());
        let serialized = serde_json::to_string(&state).unwrap();
        let mut recovered: AppData = serde_json::from_str(&serialized).unwrap();
        merge_imported_events(&mut recovered, incoming.clone()).unwrap();
        assert!(recovered.events.is_empty());
        recovered.calendar_tombstones.clear();
        merge_imported_events(&mut recovered, incoming.clone()).unwrap();
        assert_eq!(recovered.events.len(), 3);
        let canceled_id = incoming[1].id.clone();
        crate::remember_calendar_cancellation(&mut recovered, &canceled_id).unwrap();
        recovered.events.retain(|entry| entry.id != canceled_id);
        merge_imported_events(&mut recovered, incoming).unwrap();
        assert_eq!(recovered.events.len(), 2);
    }

    #[test]
    fn source_series_metadata_keeps_rrule_exceptions_and_user_edits_across_json() {
        let input = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:source17@example.org\nSUMMARY:Imported plan\nDESCRIPTION:Private note\nDTSTART:20270614T113000Z\nDTEND:20270614T123000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4\nEXDATE:20270616T113000Z\nRDATE:20270630T113000Z\nEND:VEVENT\nEND:VCALENDAR";
        let (occurrences, sources) = parse_ics_bundle(input, None).unwrap();
        assert_eq!(occurrences.len(), 4);
        assert_eq!(sources.len(), 1);
        let mut state = AppData::default();
        merge_imported_events(&mut state, occurrences.clone()).unwrap();
        for source in sources {
            assert!(source.valid(&source.uid));
            state.calendar_series.insert(source.uid.clone(), source);
        }
        let key = "ics:source17@example.org";
        let source = state.calendar_series.get(key).unwrap();
        assert_eq!(source.schema_version, 1);
        assert_eq!(
            source.rrule.as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4")
        );
        assert_eq!(source.exdates.len(), 1);
        assert_eq!(source.rdates.len(), 1);
        assert_eq!(source.dtend.as_ref().unwrap().1, "20270614T123000Z");
        let changed = occurrences[1].id.clone();
        state.events[1].description = "A user edit".to_owned();
        state
            .calendar_series
            .get_mut(key)
            .unwrap()
            .modified_occurrence_ids
            .insert(changed);
        let json = serde_json::to_string(&state).unwrap();
        let mut restored: AppData = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.calendar_series.get(key),
            state.calendar_series.get(key)
        );
        merge_imported_events(&mut restored, occurrences).unwrap();
        assert_eq!(restored.events.len(), 4);
        assert_eq!(restored.events[1].description, "A user edit");
        let first_id = restored.events[0].id.clone();
        crate::update_series_title(&mut restored, &first_id, "Retitled").unwrap();
        assert_eq!(restored.calendar_series.get(key).unwrap().title, "Retitled");
        crate::remove_series(&mut restored, &first_id).unwrap();
        assert!(restored.calendar_series.is_empty());
        assert!(restored.calendar_tombstones.contains(key));
    }

    fn source_fixture() -> AppData {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:phase18master@example.org\nSUMMARY:Safe recurrence\nDESCRIPTION:Confidential local note\nDTSTART;TZID=Asia/Jakarta:20270614T090000\nDTEND;TZID=Asia/Jakarta:20270614T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=5\nEXDATE;TZID=Asia/Jakarta:20270616T090000\nRDATE;TZID=Asia/Jakarta:20270630T090000\nBEGIN:VALARM\nACTION:DISPLAY\nTRIGGER:-PT15M\nDESCRIPTION:Reminder\nEND:VALARM\nEND:VEVENT\nEND:VCALENDAR";
        let (events, sources) = parse_ics_bundle(ics, None).unwrap();
        assert_eq!(events.len(), 5);
        let mut data = AppData::default();
        merge_imported_events(&mut data, events).unwrap();
        for source in sources {
            data.calendar_series.insert(source.uid.clone(), source);
        }
        data
    }

    #[test]
    fn phase18_original_master_rrule_export_roundtrips_all_occurrences() {
        let data = source_fixture();
        let export = serialize_ics_with_sources(&data).unwrap();
        assert_eq!(export.matches("BEGIN:VEVENT").count(), 1);
        assert!(export.contains("RRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=5"));
        assert!(export.contains("EXDATE;TZID=Asia/Jakarta:20270616T090000"));
        assert!(export.contains("RDATE;TZID=Asia/Jakarta:20270630T090000"));
        assert!(export.contains("DTSTART;TZID=Asia/Jakarta:20270614T090000"));
        assert!(export.contains("TRIGGER:-PT15M"));
        assert!(!export.contains("Confidential local note\n")); // escaped, never raw newlines
        let (restored, sources) = parse_ics_bundle(&export, None).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(restored.len(), 5);
        for expected in &data.events {
            let candidate = restored.iter().find(|item| item.id == expected.id).unwrap();
            assert_eq!(candidate.starts_at_ms, expected.starts_at_ms);
            assert_eq!(candidate.ends_at_ms, expected.ends_at_ms);
            assert_eq!(candidate.remind_minutes, expected.remind_minutes);
            assert_eq!(candidate.all_day, expected.all_day);
        }
        // JSON round-trip of the source and event list cannot change eligibility.
        let state: AppData = serde_json::from_str(&serde_json::to_string(&data).unwrap()).unwrap();
        assert_eq!(
            serialize_ics_with_sources(&state)
                .unwrap()
                .matches("BEGIN:VEVENT")
                .count(),
            1
        );
    }

    #[test]
    fn phase18_modified_occurrence_is_flattened_without_master_or_duplicate() {
        let mut data = source_fixture();
        let id = data.events[1].id.clone();
        data.events[1].description = "User-specific annotation".into();
        data.calendar_series
            .get_mut("ics:phase18master@example.org")
            .unwrap()
            .modified_occurrence_ids
            .insert(id);
        let exported = serialize_ics_with_sources(&data).unwrap();
        assert!(!exported.contains("RRULE:"));
        assert_eq!(exported.matches("BEGIN:VEVENT").count(), 5);
        let restored = parse_ics(&exported, None).unwrap();
        assert_eq!(restored.len(), 5);
        assert_eq!(
            restored
                .iter()
                .filter(|event| event.description == "User-specific annotation")
                .count(),
            1
        );
        let mut unknown_edit = source_fixture();
        unknown_edit.events[0].starts_at_ms += 60_000; // missed metadata marker
        let safely_flat = serialize_ics_with_sources(&unknown_edit).unwrap();
        assert!(!safely_flat.contains("RRULE:"));
    }

    #[test]
    fn phase18_deleted_occurrence_never_resurrects_on_source_export() {
        let mut data = source_fixture();
        let removed = data.events.remove(2);
        data.calendar_tombstones.insert(removed.id.clone());
        let exported = serialize_ics_with_sources(&data).unwrap();
        assert_eq!(exported.matches("BEGIN:VEVENT").count(), 4);
        assert!(!exported.contains("RRULE:"));
        let restored = parse_ics(&exported, None).unwrap();
        assert_eq!(restored.len(), 4);
        assert!(!restored.iter().any(|event| event.id == removed.id));
        let mut canceled = source_fixture();
        canceled
            .calendar_tombstones
            .insert("ics:phase18master@example.org".into());
        assert!(!serialize_ics_with_sources(&canceled)
            .unwrap()
            .contains("RRULE:"));
    }

    #[test]
    fn phase18_source_export_keeps_global_rename_and_all_day_rules() {
        let mut data = source_fixture();
        let id = data.events[0].id.clone();
        crate::update_series_title(&mut data, &id, "Renamed series").unwrap();
        let exported = serialize_ics_with_sources(&data).unwrap();
        assert!(exported.contains("RRULE:"));
        assert!(exported.contains("SUMMARY:Renamed series"));
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:phase18allday@example.org\nSUMMARY:Leap day\nDTSTART;VALUE=DATE:20280229\nDTEND;VALUE=DATE:20280302\nRRULE:FREQ=YEARLY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let (events, sources) = parse_ics_bundle(ics, None).unwrap();
        let state = AppData {
            events,
            calendar_series: sources
                .into_iter()
                .map(|source| (source.uid.clone(), source))
                .collect(),
            ..AppData::default()
        };
        let text = serialize_ics_with_sources(&state).unwrap();
        assert!(text.contains("RRULE:FREQ=YEARLY;COUNT=3"));
        assert!(text.contains("DTSTART;VALUE=DATE:20280229"));
        assert_eq!(parse_ics(&text, None).unwrap().len(), 3);
    }

    #[test]
    fn phase18_legacy_alarm_or_corrupt_source_metadata_falls_back_safely() {
        let mut data = source_fixture();
        data.calendar_series
            .get_mut("ics:phase18master@example.org")
            .unwrap()
            .reminder_minutes = None;
        let exported = serialize_ics_with_sources(&data).unwrap();
        assert!(!exported.contains("RRULE:"));
        assert_eq!(exported.matches("BEGIN:VALARM").count(), 5);
        let mut unsafe_source = source_fixture();
        unsafe_source
            .calendar_series
            .get_mut("ics:phase18master@example.org")
            .unwrap()
            .rrule = Some("FREQ=DAILY;COUNT=3\nDESCRIPTION:Injected".into());
        let text = serialize_ics_with_sources(&unsafe_source).unwrap();
        assert!(!text.contains("Injected"));
        assert!(!text.contains("RRULE:"));
        assert_eq!(parse_ics(&text, None).unwrap().len(), 5);
    }

    #[test]
    fn complex_monthly_and_yearly_ics_are_bounded_and_export_flattens() {
        let monthly = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:monthly17@example.org\nSUMMARY:Every last Monday\nDTSTART:20270125T090000Z\nRRULE:FREQ=MONTHLY;BYDAY=-1MO;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let (events, source) = parse_ics_bundle(monthly, None).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0].starts_at_ms,
            utc_to_ms("20270125T090000Z").unwrap()
        );
        assert_eq!(
            events[1].starts_at_ms,
            utc_to_ms("20270222T090000Z").unwrap()
        );
        assert_eq!(
            events[2].starts_at_ms,
            utc_to_ms("20270329T090000Z").unwrap()
        );
        assert_eq!(
            source[0].rrule.as_deref(),
            Some("FREQ=MONTHLY;BYDAY=-1MO;COUNT=3")
        );
        assert!(!serialize_ics(&events).unwrap().contains("RRULE:"));
        let yearly = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:leap17@example.org\nSUMMARY:Leap\nDTSTART;VALUE=DATE:20280229\nRRULE:FREQ=YEARLY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let days = parse_ics(yearly, None).unwrap();
        assert_eq!(
            days.iter()
                .map(|e| e.all_day.as_ref().unwrap().start_date.as_str())
                .collect::<Vec<_>>(),
            vec!["2028-02-29", "2032-02-29", "2036-02-29"]
        );
    }

    #[test]
    fn all_day_date_and_exclusive_end_roundtrip_without_timezone_shift() {
        let input = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:festival@example.org\nSUMMARY:Leap day event\nDTSTART;VALUE=DATE:20280229\nDTEND;VALUE=DATE:20280302\nEND:VEVENT\nEND:VCALENDAR";
        let items = parse_ics(input, Some("Pacific/Honolulu")).unwrap();
        assert_eq!(items.len(), 1);
        let span = items[0].all_day.as_ref().unwrap();
        assert_eq!(span.start_date, "2028-02-29");
        assert_eq!(span.end_date_exclusive, "2028-03-02");
        assert_eq!(items[0].remind_minutes, None);
        let serialized = serialize_ics(&items).unwrap();
        assert!(serialized.contains("DTSTART;VALUE=DATE:20280229"));
        assert!(serialized.contains("DTEND;VALUE=DATE:20280302"));
        assert!(!serialized.contains("T000000Z"));
        let other = parse_ics(&serialized, Some("Asia/Jakarta")).unwrap();
        assert_eq!(items[0].all_day, other[0].all_day);
        assert_eq!(items[0].starts_at_ms, other[0].starts_at_ms);
        assert_eq!(items[0].id, other[0].id);
    }

    #[test]
    fn date_only_rrule_exdate_rdate_and_series_identity_are_stable() {
        let input = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:dates@example.org\nSUMMARY:Holiday sequence\nDTSTART;VALUE=DATE:20270614\nDTEND;VALUE=DATE:20270616\nRRULE:FREQ=DAILY;COUNT=3\nEXDATE;VALUE=DATE:20270615\nRDATE;VALUE=DATE:20270620\nEND:VEVENT\nEND:VCALENDAR";
        let items = parse_ics(input, None).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(
            items
                .iter()
                .map(|e| e.all_day.as_ref().unwrap().start_date.as_str())
                .collect::<Vec<_>>(),
            vec!["2027-06-14", "2027-06-16", "2027-06-20"]
        );
        assert_eq!(
            items[1].all_day.as_ref().unwrap().end_date_exclusive,
            "2027-06-18"
        );
        assert_eq!(
            items[2].all_day.as_ref().unwrap().end_date_exclusive,
            "2027-06-22"
        );
        assert!(items
            .iter()
            .all(|event| event.series_uid.as_deref() == Some("ics:dates@example.org")));
        let mut data = AppData::default();
        merge_imported_events(&mut data, items.clone()).unwrap();
        merge_imported_events(&mut data, items).unwrap();
        assert_eq!(data.events.len(), 3);
        let exported = serialize_ics(&data.events).unwrap();
        assert_eq!(parse_ics(&exported, None).unwrap().len(), 3);
        assert!(!exported.contains("RRULE:"));
    }

    #[test]
    fn timed_exception_lists_resolve_dst_and_reject_invalid_values() {
        let input = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:timed@example.org\nSUMMARY:NY schedule\nDTSTART;TZID=America/New_York:20270313T093000\nRRULE:FREQ=DAILY;COUNT=3\nEXDATE;TZID=America/New_York:20270314T093000\nRDATE;TZID=America/New_York:20270320T093000\nEND:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(input, None).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0].starts_at_ms,
            utc_to_ms("20270313T143000Z").unwrap()
        );
        assert_eq!(
            events[1].starts_at_ms,
            utc_to_ms("20270315T133000Z").unwrap()
        );
        assert_eq!(
            events[2].starts_at_ms,
            utc_to_ms("20270320T133000Z").unwrap()
        );
        let malformed = input.replace(
            "RDATE;TZID=America/New_York:20270320T093000",
            "RDATE;TZID=Unknown/Place:20270320T093000",
        );
        assert!(parse_ics(&malformed, None).is_err());
        let invalid_date = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Impossible day\nDTSTART;VALUE=DATE:20270229\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(invalid_date, None).is_err());
    }

    #[test]
    fn import_merge_is_idempotent_and_preserves_legacy_utc_state() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:stable@example.org\nSUMMARY:Daily meeting\nDTSTART:20270614T113000Z\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT\nEND:VCALENDAR";
        let parsed = parse_ics(ics, None).unwrap();
        let mut state = AppData::default();
        state.events.push(CalendarEvent {
            id: "legacy".into(),
            title: "Existing calendar event".into(),
            starts_at_ms: utc_to_ms("20270601T120000Z").unwrap(),
            description: String::new(),
            remind_minutes: Some(15),
            created_ms: 0,
            all_day: None,
            series_uid: None,
            ends_at_ms: None,
        });
        merge_imported_events(&mut state, parsed.clone()).unwrap();
        assert_eq!(state.events.len(), 4);
        merge_imported_events(&mut state, parsed).unwrap();
        assert_eq!(state.events.len(), 4);
        assert_eq!(state.events[0].id, "legacy");
        let serialized = serde_json::to_string(&state).unwrap();
        let recovered: AppData = serde_json::from_str(&serialized).unwrap();
        assert_eq!(recovered.events.len(), 4);
        assert_eq!(recovered.events[0].title, "Existing calendar event");
    }

    #[test]
    fn malformed_or_unknown_tz_is_rejected() {
        assert!(parse_ics("BEGIN:VCALENDAR\r\nEND:VCALENDAR", None).is_err());
        let supported = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Meeting\nDTSTART;TZID=Asia/Jakarta:20270614T113000\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(supported, None).is_ok());
        let unsupported = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Meeting\nDTSTART;TZID=Unknown/Timezone:20270614T113000\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(unsupported, None).is_err());
    }
}
