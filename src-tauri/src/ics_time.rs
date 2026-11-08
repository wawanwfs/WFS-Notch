//! Safe local iCalendar DTSTART and bounded RRULE conversion.
//! Persisted events always contain UTC epoch milliseconds. We resolve IANA
//! timezone rules on import; we never infer a floating timezone silently.
use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Timelike, Weekday};
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
        let Some(text) = rrule else {
            return Ok(vec![self.instant(self.local)?]);
        };
        let rule = Recurrence::parse(text)?;
        let start_day = self.local.date();
        if !rule.matches(start_day, start_day)? {
            return Err("RRULE filters do not include DTSTART; inconsistent series rejected".into());
        }
        let mut occurrences = Vec::new();
        let mut finished_by_until = false;
        // The app only accepts calendar dates 2000..2099. Scan at most 100
        // years of civil days: bounded even for sparse leap-day/yearly rules.
        for offset in 0..=(366 * 100_i64) {
            let day = start_day
                .checked_add_signed(Duration::days(offset))
                .ok_or("RRULE date exceeds supported calendar range")?;
            if day.year() > 2099 {
                break;
            }
            if !rule.matches(day, start_day)? {
                continue;
            }
            let wall = day
                .and_hms_opt(self.local.hour(), self.local.minute(), self.local.second())
                .ok_or("Invalid recurrence wall-clock time")?;
            let instant = self.instant(wall)?;
            if rule.until.is_some_and(|limit| instant > limit) {
                finished_by_until = true;
                break;
            }
            if occurrences.len() == MAX_OCCURRENCES {
                return Err("RRULE exceeds 250 occurrences; refuse partial import".into());
            }
            occurrences.push(instant);
            if rule.count.is_some_and(|count| occurrences.len() == count) {
                return Ok(occurrences);
            }
        }
        if occurrences.is_empty() {
            return Err("RRULE has no valid occurrences after DTSTART".into());
        }
        // COUNT without UNTIL must be achieved within 2000..2099. Returning a
        // truncated series would silently change a user's scheduling intent.
        if rule.count.is_some() && rule.until.is_none() {
            return Err("RRULE COUNT exceeds supported date range; no partial import".into());
        }
        if rule.until.is_some() && !finished_by_until
            && rule.count.is_none()
            && rule.until.unwrap_or(0) > self.instant(
                NaiveDate::from_ymd_opt(2099, 12, 31)
                    .ok_or("Invalid range end")?
                    .and_hms_opt(23, 59, 59)
                    .ok_or("Invalid range time")?
            )?
        {
            return Err("RRULE UNTIL exceeds supported date range".into());
        }
        Ok(occurrences)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Copy, Debug)]
struct WeekdaySelector {
    day: Weekday,
    ordinal: Option<i8>,
}

fn parse_weekday(value: &str) -> Result<Weekday, String> {
    match value {
        "MO" => Ok(Weekday::Mon),
        "TU" => Ok(Weekday::Tue),
        "WE" => Ok(Weekday::Wed),
        "TH" => Ok(Weekday::Thu),
        "FR" => Ok(Weekday::Fri),
        "SA" => Ok(Weekday::Sat),
        "SU" => Ok(Weekday::Sun),
        _ => Err("Invalid BYDAY/WKST weekday".into()),
    }
}

fn parse_rule_values<T>(
    raw: &str,
    max: usize,
    mut parse: impl FnMut(&str) -> Result<T, String>,
) -> Result<Vec<T>, String>
where
    T: Eq + std::hash::Hash + Copy,
{
    if raw.is_empty() || raw.split(',').count() > max {
        return Err("RRULE list is empty or too long".into());
    }
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for entry in raw.split(',') {
        let value = parse(entry)?;
        if !seen.insert(value) {
            return Err("RRULE list contains duplicates".into());
        }
        values.push(value);
    }
    Ok(values)
}

struct Recurrence {
    freq: Frequency,
    interval: i64,
    count: Option<usize>,
    until: Option<u64>,
    weekdays: Vec<WeekdaySelector>,
    months: Vec<u32>,
    month_days: Vec<i32>,
    week_start: Weekday,
}

impl Recurrence {
    fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 512 {
            return Err("RRULE exceeds 512 bytes".into());
        }
        let mut freq = None;
        let mut interval = 1_i64;
        let mut count = None;
        let mut until = None;
        let mut weekdays = Vec::new();
        let mut months = Vec::new();
        let mut month_days = Vec::new();
        let mut week_start = Weekday::Mon;
        let mut found = HashSet::new();
        for section in text.split(';') {
            let (key, raw) = section
                .split_once('=')
                .ok_or("Malformed RRULE parameter")?;
            let key = key.to_ascii_uppercase();
            if !found.insert(key.clone()) {
                return Err("Duplicate RRULE parameter".into());
            }
            match key.as_str() {
                "FREQ" => {
                    freq = Some(match raw.to_ascii_uppercase().as_str() {
                        "DAILY" => Frequency::Daily,
                        "WEEKLY" => Frequency::Weekly,
                        "MONTHLY" => Frequency::Monthly,
                        "YEARLY" => Frequency::Yearly,
                        _ => return Err("Unsupported RRULE FREQ".into()),
                    })
                }
                "INTERVAL" => {
                    interval = raw.parse().map_err(|_| "Invalid RRULE INTERVAL")?;
                    if !(1..=365).contains(&interval) {
                        return Err("RRULE INTERVAL must be 1..365".into());
                    }
                }
                "COUNT" => {
                    let n: usize = raw.parse().map_err(|_| "Invalid RRULE COUNT")?;
                    if !(1..=MAX_OCCURRENCES).contains(&n) {
                        return Err("RRULE COUNT must be 1..250".into());
                    }
                    count = Some(n);
                }
                "UNTIL" => {
                    let wall = local_datetime(
                        raw.strip_suffix('Z').ok_or("RRULE UNTIL must be explicit UTC")?,
                    )?;
                    until = Some(
                        u64::try_from(wall.and_utc().timestamp_millis())
                            .map_err(|_| "RRULE UNTIL is outside the supported range")?,
                    );
                }
                "BYDAY" => {
                    // Supported numeric ordinals are month-relative only.
                    let entries = raw.split(',').collect::<Vec<_>>();
                    if entries.is_empty() || entries.len() > 31 {
                        return Err("Invalid BYDAY list length".into());
                    }
                    let mut seen = HashSet::new();
                    for entry in entries {
                        if entry.len() < 2 {
                            return Err("Malformed BYDAY selector".into());
                        }
                        let (ordinal_raw, weekday_raw) = entry.split_at(entry.len() - 2);
                        let day = parse_weekday(&weekday_raw.to_ascii_uppercase())?;
                        let ordinal = if ordinal_raw.is_empty() {
                            None
                        } else {
                            let value: i8 = ordinal_raw
                                .parse()
                                .map_err(|_| "Invalid BYDAY ordinal")?;
                            if value == 0 || !(-5..=5).contains(&value) {
                                return Err("BYDAY ordinal must be -5..-1 or 1..5".into());
                            }
                            Some(value)
                        };
                        if !seen.insert((day, ordinal)) {
                            return Err("Duplicate BYDAY selector".into());
                        }
                        weekdays.push(WeekdaySelector { day, ordinal });
                    }
                }
                "BYMONTH" => {
                    months = parse_rule_values(raw, 12, |part| {
                        let n: u32 = part.parse().map_err(|_| "Invalid BYMONTH")?;
                        if !(1..=12).contains(&n) {
                            return Err("BYMONTH must be between 1 and 12".into());
                        }
                        Ok(n)
                    })?;
                }
                "BYMONTHDAY" => {
                    month_days = parse_rule_values(raw, 62, |part| {
                        let n: i32 = part.parse().map_err(|_| "Invalid BYMONTHDAY")?;
                        if n == 0 || !(-31..=31).contains(&n) {
                            return Err("BYMONTHDAY must be -31..-1 or 1..31".into());
                        }
                        Ok(n)
                    })?;
                }
                "WKST" => week_start = parse_weekday(&raw.to_ascii_uppercase())?,
                _ => return Err(format!("Unsupported RRULE part {key}; nothing imported")),
            }
        }
        let freq = freq.ok_or("RRULE requires FREQ")?;
        if count.is_none() && until.is_none() {
            return Err("Unbounded RRULE requires COUNT or UTC UNTIL".into());
        }
        if weekdays.iter().any(|rule| rule.ordinal.is_some()) {
            if !matches!(freq, Frequency::Monthly | Frequency::Yearly) {
                return Err("Numbered BYDAY requires MONTHLY or YEARLY frequency".into());
            }
            if freq == Frequency::Yearly && months.is_empty() {
                return Err(
                    "Year-relative numeric BYDAY without BYMONTH is not supported".into(),
                );
            }
        }
        Ok(Self {
            freq, interval, count, until, weekdays, months, month_days, week_start,
        })
    }

    fn matches(&self, day: NaiveDate, origin: NaiveDate) -> Result<bool, String> {
        let days = day.signed_duration_since(origin).num_days();
        if days < 0 {
            return Ok(false);
        }
        let month_delta = i64::from(day.year() - origin.year()) * 12
            + i64::from(day.month()) - i64::from(origin.month());
        let active_period = match self.freq {
            Frequency::Daily => days % self.interval == 0,
            Frequency::Weekly => {
                let offset = (i64::from(origin.weekday().num_days_from_monday())
                    - i64::from(self.week_start.num_days_from_monday()))
                    .rem_euclid(7);
                let since_week_start = days + offset;
                (since_week_start / 7) % self.interval == 0
            }
            Frequency::Monthly => month_delta >= 0 && month_delta % self.interval == 0,
            Frequency::Yearly => {
                let years = i64::from(day.year() - origin.year());
                years >= 0 && years % self.interval == 0
            }
        };
        if !active_period {
            return Ok(false);
        }
        if !self.months.is_empty() && !self.months.contains(&day.month()) {
            return Ok(false);
        }
        let last = last_day_of_month(day.year(), day.month())?;
        if !self.month_days.is_empty() && !self.month_days.iter().any(|n| {
            let wanted = if *n > 0 { *n } else { last as i32 + 1 + *n };
            i32::try_from(day.day()).is_ok_and(|d| d == wanted)
        }) {
            return Ok(false);
        }
        if !self.weekdays.is_empty() {
            if !self.weekdays.iter().any(|selected| {
                if selected.day != day.weekday() {
                    return false;
                }
                match selected.ordinal {
                    None => true,
                    Some(n) if n > 0 => ((day.day() - 1) / 7 + 1) as i8 == n,
                    Some(n) => -(((last - day.day()) / 7 + 1) as i8) == n,
                }
            }) {
                return Ok(false);
            }
        } else if self.freq == Frequency::Weekly && day.weekday() != origin.weekday() {
            return Ok(false);
        }

        if matches!(self.freq, Frequency::Monthly | Frequency::Yearly)
            && self.weekdays.is_empty()
            && self.month_days.is_empty()
            && day.day() != origin.day()
        {
            return Ok(false);
        }
        if self.freq == Frequency::Yearly
            && self.months.is_empty()
            && self.weekdays.is_empty()
            && self.month_days.is_empty()
            && day.month() != origin.month()
        {
            return Ok(false);
        }
        Ok(true)
    }
}

fn last_day_of_month(year: i32, month: u32) -> Result<u32, String> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1).ok_or("RRULE year overflow")?, 1)
    } else {
        (year, month + 1)
    };
    let next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .ok_or("RRULE has invalid month")?;
    Ok(next.pred_opt().ok_or("RRULE day underflow")?.day())
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
        assert_eq!(
            start.occurrences(Some("FREQ=MONTHLY;COUNT=2")).unwrap(),
            vec![ms("20270614T113000Z"), ms("20270714T113000Z")]
        );
        assert_eq!(
            start.occurrences(Some("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=3")).unwrap(),
            vec![ms("20270614T113000Z"), ms("20270616T113000Z"), ms("20270621T113000Z")]
        );
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
    fn monthly_recurrence_skips_short_months_instead_of_clamping() {
        let jan = CalendarStart::parse("DTSTART", "20270131T120000Z", None).unwrap();
        assert_eq!(
            jan.occurrences(Some("FREQ=MONTHLY;COUNT=3")).unwrap(),
            vec![ms("20270131T120000Z"), ms("20270331T120000Z"), ms("20270531T120000Z")]
        );
        let leap = CalendarStart::parse("DTSTART", "20280229T120000Z", None).unwrap();
        assert_eq!(
            leap.occurrences(Some("FREQ=YEARLY;COUNT=3")).unwrap(),
            vec![ms("20280229T120000Z"), ms("20320229T120000Z"), ms("20360229T120000Z")]
        );
    }

    #[test]
    fn monthly_ordinal_and_negative_monthday_are_deterministic() {
        let first = CalendarStart::parse("DTSTART", "20270104T080000Z", None).unwrap();
        assert_eq!(
            first.occurrences(Some("FREQ=MONTHLY;BYDAY=1MO;COUNT=3")).unwrap(),
            vec![ms("20270104T080000Z"), ms("20270201T080000Z"), ms("20270301T080000Z")]
        );
        let last = CalendarStart::parse("DTSTART", "20270131T080000Z", None).unwrap();
        assert_eq!(
            last.occurrences(Some("FREQ=MONTHLY;BYMONTHDAY=-1;COUNT=3")).unwrap(),
            vec![ms("20270131T080000Z"), ms("20270228T080000Z"), ms("20270331T080000Z")]
        );
    }

    #[test]
    fn weekly_wkst_and_multi_day_byday_preserve_local_time() {
        let start = CalendarStart::parse("DTSTART;TZID=America/New_York", "20270313T093000", None).unwrap();
        assert_eq!(
            start.occurrences(Some("FREQ=WEEKLY;INTERVAL=2;WKST=MO;BYDAY=SA,SU;COUNT=4")).unwrap(),
            vec![
                ms("20270313T143000Z"),
                ms("20270314T133000Z"),
                ms("20270327T133000Z"),
                ms("20270328T133000Z")
            ]
        );
    }

    #[test]
    fn yearly_by_month_and_by_day_filters_respect_bounds() {
        let start = CalendarStart::parse("DTSTART", "20270614T120000Z", None).unwrap();
        assert_eq!(
            start.occurrences(Some("FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=14;COUNT=3")).unwrap(),
            vec![ms("20270614T120000Z"), ms("20280614T120000Z"), ms("20290614T120000Z")]
        );
        for rule in [
            "FREQ=MONTHLY;COUNT=3;BYSETPOS=1",
            "FREQ=WEEKLY;BYDAY=1MO;COUNT=3",
            "FREQ=YEARLY;BYDAY=-1MO;COUNT=3",
            "FREQ=MONTHLY;BYMONTHDAY=0;COUNT=3",
            "FREQ=YEARLY;BYMONTH=13;COUNT=3",
            "FREQ=YEARLY;WKST=XX;COUNT=3",
            "FREQ=MONTHLY;BYDAY=0MO;COUNT=3",
        ] {
            assert!(start.occurrences(Some(rule)).is_err(), "Invalid/unsupported {rule}");
        }
    }

    #[test]
    fn all_day_dates_are_never_silently_converted_to_timed_events() {
        assert!(CalendarStart::parse("DTSTART;VALUE=DATE", "20270614", Some("UTC")).is_err());
        assert!(CalendarStart::parse("DTSTART", "20270614", Some("UTC")).is_err());
    }
}
