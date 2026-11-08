//! Safe local iCalendar DTSTART and bounded RRULE conversion.
//! Persisted events always contain UTC epoch milliseconds. We resolve IANA
//! timezone rules on import; we never infer a floating timezone silently.
use chrono::{Duration, LocalResult, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use std::collections::HashSet;

const MAX_OCCURRENCES: usize = 250;

#[derive(Clone)]
pub(super) struct CalendarStart {
    local: NaiveDateTime,
    zone: Option<Tz>,
}

fn local_datetime(value: &str) -> Result<NaiveDateTime, String> {
    if value.len() != 15 || value.as_bytes()[8] != b'T' {
        return Err("Invalid iCalendar date-time; expected YYYYMMDDTHHMMSS".into());
    }
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|_| "Invalid iCalendar calendar date or wall-clock time".into())
}

fn iana_zone(name: &str) -> Result<Tz, String> {
    if name.len() > 80 || name.is_empty() {
        return Err("TZID must be a valid IANA timezone identifier".into());
    }
    name.parse::<Tz>().map_err(|_| {
        "TZID is not in the IANA database; no silent local-time conversion was made".into()
    })
}

impl CalendarStart {
    pub(super) fn parse(
        full_key: &str,
        value: &str,
        floating_timezone: Option<&str>,
    ) -> Result<Self, String> {
        let mut zone: Option<Tz> = None;
        let mut is_date = false;
        for option in full_key.split(';').skip(1) {
            let (key, raw) = option
                .split_once('=')
                .ok_or("Malformed DTSTART parameter")?;
            let raw = raw.trim_matches('"');
            if key.eq_ignore_ascii_case("TZID") {
                if zone.is_some() {
                    return Err("Duplicate DTSTART TZID".into());
                }
                zone = Some(iana_zone(raw)?);
            } else if key.eq_ignore_ascii_case("VALUE") {
                if raw.eq_ignore_ascii_case("DATE") {
                    is_date = true;
                } else if !raw.eq_ignore_ascii_case("DATE-TIME") {
                    return Err("Unsupported DTSTART VALUE parameter".into());
                }
            } else {
                return Err("Unsupported DTSTART parameter; cannot safely infer timezone".into());
            }
        }
        // All-day dates require a persisted all-day flag and end-date semantics.
        // Treating DATE as midnight timed events would silently change meaning.
        if is_date || (value.len() == 8 && value.bytes().all(|b| b.is_ascii_digit())) {
            return Err(
                "All-day VALUE=DATE events are not supported yet; no silent midnight conversion"
                    .into(),
            );
        }
        let utc = value.ends_with('Z');
        if utc && zone.is_some() {
            return Err("DTSTART cannot combine UTC Z and TZID".into());
        }
        if utc {
            let body = value.strip_suffix('Z').ok_or("Malformed UTC DTSTART")?;
            return Ok(Self {
                local: local_datetime(body)?,
                zone: None,
            });
        }
        let local = local_datetime(value)?;
        let zone = match zone {
            Some(value) => value,
            None => iana_zone(
                floating_timezone
                    .filter(|value| !value.trim().is_empty())
                    .ok_or("Floating DTSTART requires an explicit IANA timezone from the user")?,
            )?,
        };
        Ok(Self {
            local,
            zone: Some(zone),
        })
    }

    fn instant(&self, wall_time: NaiveDateTime) -> Result<u64, String> {
        let milliseconds = if let Some(zone) = self.zone {
            match zone.from_local_datetime(&wall_time) {
                LocalResult::Single(value) => value.timestamp_millis(),
                LocalResult::None => {
                    return Err(
                        "Nonexistent wall time during DST transition; event not imported".into(),
                    )
                }
                LocalResult::Ambiguous(_, _) => {
                    return Err(
                        "Ambiguous wall time during DST transition; event not imported".into(),
                    )
                }
            }
        } else {
            wall_time.and_utc().timestamp_millis()
        };
        u64::try_from(milliseconds)
            .map_err(|_| "Event time is outside the supported UTC epoch range".into())
    }

    pub(super) fn occurrences(&self, rrule: Option<&str>) -> Result<Vec<u64>, String> {
        let Some(rule) = rrule else {
            return Ok(vec![self.instant(self.local)?]);
        };
        let recurrence = Recurrence::parse(rule)?;
        let mut occurrences = Vec::new();
        // If COUNT is omitted, test one extra occurrence to detect truncation.
        let iterations = recurrence.count.unwrap_or(MAX_OCCURRENCES + 1);
        for index in 0..iterations {
            let days = (index as i64)
                .checked_mul(recurrence.step_days)
                .ok_or("Recurrence interval overflow")?;
            let wall = self
                .local
                .checked_add_signed(Duration::days(days))
                .ok_or("Recurrence exceeds valid dates")?;
            let instant = self.instant(wall)?;
            if recurrence.until.is_some_and(|limit| instant > limit) {
                break;
            }
            if occurrences.len() >= MAX_OCCURRENCES {
                return Err(
                    "RRULE would import more than 250 occurrences; refuse partial import".into(),
                );
            }
            occurrences.push(instant);
        }
        if occurrences.is_empty() {
            return Err("RRULE UNTIL occurs before DTSTART".into());
        }
        Ok(occurrences)
    }
}

struct Recurrence {
    step_days: i64,
    count: Option<usize>,
    until: Option<u64>,
}

impl Recurrence {
    fn parse(text: &str) -> Result<Self, String> {
        let mut freq: Option<String> = None;
        let mut interval: i64 = 1;
        let mut count: Option<usize> = None;
        let mut until: Option<u64> = None;
        let mut found = HashSet::new();
        for section in text.split(';') {
            let (key, raw) = section.split_once('=').ok_or("Malformed RRULE parameter")?;
            let key = key.to_ascii_uppercase();
            if !found.insert(key.clone()) {
                return Err("Duplicate RRULE parameter".into());
            }
            match key.as_str() {
                "FREQ" => freq = Some(raw.to_ascii_uppercase()),
                "INTERVAL" => {
                    interval = raw.parse::<i64>().map_err(|_| "Invalid RRULE INTERVAL")?;
                    if !(1..=365).contains(&interval) {
                        return Err("RRULE INTERVAL must be between 1 and 365".into());
                    }
                }
                "COUNT" => {
                    let value = raw.parse::<usize>().map_err(|_| "Invalid RRULE COUNT")?;
                    if !(1..=MAX_OCCURRENCES).contains(&value) {
                        return Err("RRULE COUNT must be between 1 and 250".into());
                    }
                    count = Some(value);
                }
                "UNTIL" => {
                    let body = raw
                        .strip_suffix('Z')
                        .ok_or("RRULE UNTIL must be an explicit UTC timestamp")?;
                    let wall = local_datetime(body)?;
                    until = Some(
                        u64::try_from(wall.and_utc().timestamp_millis())
                            .map_err(|_| "RRULE UNTIL is outside the supported range")?,
                    );
                }
                _ => {
                    return Err(format!(
                        "Unsupported RRULE part {key}; nothing was imported"
                    ))
                }
            }
        }
        let frequency = freq.ok_or("RRULE must specify FREQ")?;
        let days_per_cycle = match frequency.as_str() {
            "DAILY" => 1_i64,
            "WEEKLY" => 7_i64,
            _ => {
                return Err(
                    "Only simple DAILY/WEEKLY RRULE without BYDAY/BYMONTH is supported".into(),
                )
            }
        };
        if count.is_none() && until.is_none() {
            return Err("Unbounded RRULE requires COUNT or UTC UNTIL".into());
        }
        Ok(Self {
            step_days: days_per_cycle * interval,
            count,
            until,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(timestamp: &str) -> u64 {
        let without_z = timestamp.strip_suffix('Z').unwrap();
        local_datetime(without_z)
            .unwrap()
            .and_utc()
            .timestamp_millis() as u64
    }

    #[test]
    fn jakarta_iana_time_and_explicit_floating_policy() {
        let zoned =
            CalendarStart::parse("DTSTART;TZID=Asia/Jakarta", "20270614T113000", None).unwrap();
        assert_eq!(
            zoned.occurrences(None).unwrap(),
            vec![ms("20270614T043000Z")]
        );
        assert!(CalendarStart::parse("DTSTART", "20270614T113000", None).is_err());
        let floating =
            CalendarStart::parse("DTSTART", "20270614T113000", Some("Asia/Jakarta")).unwrap();
        assert_eq!(
            floating.occurrences(None).unwrap(),
            zoned.occurrences(None).unwrap()
        );
        assert!(
            CalendarStart::parse("DTSTART;TZID=Unknown/Zone", "20270614T113000", None).is_err()
        );
        assert!(
            CalendarStart::parse("DTSTART;TZID=Asia/Jakarta", "20270614T043000Z", None).is_err()
        );
    }

    #[test]
    fn dst_ambiguous_and_nonexistent_local_times_are_refused() {
        let missing =
            CalendarStart::parse("DTSTART;TZID=America/New_York", "20270314T023000", None).unwrap();
        assert!(missing
            .occurrences(None)
            .unwrap_err()
            .contains("Nonexistent"));
        let ambiguous =
            CalendarStart::parse("DTSTART;TZID=America/New_York", "20271107T013000", None).unwrap();
        assert!(ambiguous
            .occurrences(None)
            .unwrap_err()
            .contains("Ambiguous"));
        let before =
            CalendarStart::parse("DTSTART;TZID=America/New_York", "20270314T013000", None).unwrap();
        assert_eq!(
            before.occurrences(None).unwrap(),
            vec![ms("20270314T063000Z")]
        );
    }

    #[test]
    fn recurrence_keeps_wall_time_across_dst_boundary() {
        let start =
            CalendarStart::parse("DTSTART;TZID=America/New_York", "20270313T093000", None).unwrap();
        assert_eq!(
            start.occurrences(Some("FREQ=DAILY;COUNT=3")).unwrap(),
            vec![
                ms("20270313T143000Z"),
                ms("20270314T133000Z"),
                ms("20270315T133000Z"),
            ]
        );
        let weekly = CalendarStart::parse("DTSTART", "20270614T113000Z", None).unwrap();
        assert_eq!(
            weekly
                .occurrences(Some("FREQ=WEEKLY;INTERVAL=2;COUNT=2"))
                .unwrap(),
            vec![ms("20270614T113000Z"), ms("20270628T113000Z")]
        );
    }

    #[test]
    fn recurrence_bounds_and_unknown_parts_fail_closed() {
        let start = CalendarStart::parse("DTSTART", "20270614T113000Z", None).unwrap();
        assert!(start.occurrences(Some("FREQ=DAILY")).is_err());
        assert!(start.occurrences(Some("FREQ=DAILY;COUNT=251")).is_err());
        assert!(start
            .occurrences(Some("FREQ=DAILY;UNTIL=20991231T235959Z"))
            .is_err());
        assert!(start.occurrences(Some("FREQ=MONTHLY;COUNT=2")).is_err());
        assert!(start
            .occurrences(Some("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=3"))
            .is_err());
        assert_eq!(
            start
                .occurrences(Some("FREQ=DAILY;UNTIL=20270615T110000Z"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            start
                .occurrences(Some("FREQ=DAILY;COUNT=2;UNTIL=20270615T200000Z"))
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn all_day_dates_are_never_silently_converted_to_timed_events() {
        assert!(CalendarStart::parse("DTSTART;VALUE=DATE", "20270614", Some("UTC")).is_err());
        assert!(CalendarStart::parse("DTSTART", "20270614", Some("UTC")).is_err());
    }
}
