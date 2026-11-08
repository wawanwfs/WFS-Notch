//! Local-only, bounded RFC 5545 iCalendar interchange.
//! UTC, IANA TZID, and explicit user-selected floating timezones are resolved to UTC.
//! Simple daily/weekly recurrence is materialized as stable local event occurrences.
//! All-day dates, complex RRULE and unsupported recurrence metadata fail closed.
use crate::{
    ics_time::CalendarStart, new_id, now_ms, validate_event, with_store, AppData, CalendarEvent,
    Store,
};
use std::sync::Mutex;
use tauri::State;

const MAX_ICS_BYTES: usize = 256 * 1024;
const MAX_IMPORT_EVENTS: usize = 250;
const LOCAL_UID_SUFFIX: &str = "@wfs-notch.local";

#[derive(Default)]
struct PartialEvent {
    title: Option<String>,
    description: String,
    start: Option<CalendarStart>,
    uid: Option<String>,
    reminder: Option<u32>,
    recurrence: Option<String>,
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
        && raw.len() <= 128
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
fn parse_ics(source: &str, floating_timezone: Option<&str>) -> Result<Vec<CalendarEvent>, String> {
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
            ("END", "VEVENT") => {
                let event = current.take().ok_or("VEVENT end without start")?;
                let title = event.title.ok_or("VEVENT is missing SUMMARY")?;
                let start = event.start.ok_or("VEVENT is missing DTSTART")?;
                // Recurring UID + original instant is a stable local occurrence ID.
                // Flattening is explicit: export produces individual UTC events.
                let base_id = event_id(event.uid.as_deref());
                for (index, starts_at_ms) in start
                    .occurrences(event.recurrence.as_deref())?
                    .into_iter()
                    .enumerate()
                {
                    validate_event(&title, &event.description, starts_at_ms, event.reminder)?;
                    let id = if index == 0 {
                        base_id.clone()
                    } else {
                        format!("{base_id}:{starts_at_ms}")
                    };
                    events.push(CalendarEvent {
                        id,
                        title: title.clone(),
                        starts_at_ms,
                        remind_minutes: event.reminder,
                        description: event.description.clone(),
                        created_ms: now_ms(),
                    });
                    if events.len() > MAX_IMPORT_EVENTS {
                        return Err("ICS contains more than 250 expanded events".into());
                    }
                }
            }
            ("BEGIN", "VALARM") if current.is_some() => in_alarm = true,
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
                        "UID" => event.uid = Some(value.to_owned()),
                        "DTSTART" => {
                            if event.start.is_some() {
                                return Err("Duplicate DTSTART".into());
                            }
                            event.start =
                                Some(CalendarStart::parse(full_key, value, floating_timezone)?);
                        }
                        "RRULE" => {
                            if event.recurrence.is_some() {
                                return Err("Duplicate RRULE".into());
                            }
                            event.recurrence = Some(value.to_owned());
                        }
                        "EXDATE" | "RDATE" | "RECURRENCE-ID" => {
                            return Err(
                                "EXDATE, RDATE and RECURRENCE-ID are not yet supported; no partial import".into(),
                            );
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
    Ok(events)
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
            format!("DTSTART:{}", ms_to_utc(event.starts_at_ms)),
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
#[tauri::command]
pub fn export_ics(store: State<'_, Mutex<Store>>) -> Result<String, String> {
    let state = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    serialize_ics(&state.data.events)
}
fn merge_imported_events(data: &mut AppData, incoming: Vec<CalendarEvent>) -> Result<(), String> {
    let mut known: std::collections::HashSet<String> =
        data.events.iter().map(|event| event.id.clone()).collect();
    let fresh: Vec<_> = incoming
        .into_iter()
        .filter(|event| known.insert(event.id.clone()))
        .collect();
    if data.events.len() + fresh.len() > 1000 {
        return Err("Event limit (1000) exceeded".into());
    }
    data.events.extend(fresh);
    Ok(())
}

#[tauri::command]
pub fn import_ics(
    store: State<'_, Mutex<Store>>,
    ics_text: String,
    floating_timezone: Option<String>,
) -> Result<AppData, String> {
    let incoming = parse_ics(&ics_text, floating_timezone.as_deref())?;
    with_store(store, move |data| merge_imported_events(data, incoming))
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
    fn unsupported_exceptions_and_all_day_are_rejected_atomically() {
        for suffix in [
            "RRULE:FREQ=MONTHLY;COUNT=2\n",
            "RRULE:FREQ=DAILY;COUNT=2\nEXDATE:20270615T113000Z\n",
            "RRULE:FREQ=DAILY;COUNT=251\n",
        ] {
            let input = format!(
                "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:series@example.org\nSUMMARY:Series\nDTSTART:20270614T113000Z\n{suffix}END:VEVENT\nEND:VCALENDAR"
            );
            assert!(parse_ics(&input, None).is_err());
        }
        let all_day = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:All day\nDTSTART;VALUE=DATE:20270614\nEND:VEVENT\nEND:VCALENDAR";
        assert!(parse_ics(all_day, Some("Asia/Jakarta")).is_err());
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
