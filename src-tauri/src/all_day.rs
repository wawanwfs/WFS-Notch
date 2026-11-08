//! Calendar date-only spans. RFC 5545 DATE and DTEND use civil dates,
//! not floating midnight timestamps. End dates are EXCLUSIVE.
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct AllDaySpan {
    pub start_date: String,
    pub end_date_exclusive: String,
}

fn valid_year(date: NaiveDate) -> Result<NaiveDate, String> {
    if !(2000..=2099).contains(&date.year()) {
        return Err("All-day date must be in calendar years 2000 through 2099".into());
    }
    Ok(date)
}

pub(super) fn parse_iso_date(value: &str) -> Result<NaiveDate, String> {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return Err("Expected ISO all-day date YYYY-MM-DD".into());
    }
    valid_year(
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| "Invalid all-day calendar date")?,
    )
}

pub(super) fn parse_ics_date(full_key: &str, value: &str) -> Result<NaiveDate, String> {
    let parts: Vec<&str> = full_key.split(';').collect();
    if parts.len() != 2
        || !parts[1].eq_ignore_ascii_case("VALUE=DATE")
        || value.len() != 8
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("All-day DTSTART/DTEND require explicit VALUE=DATE and YYYYMMDD".into());
    }
    let year = value[0..4].parse::<i32>().map_err(|_| "Invalid year")?;
    let month = value[4..6].parse::<u32>().map_err(|_| "Invalid month")?;
    let day = value[6..8].parse::<u32>().map_err(|_| "Invalid day")?;
    valid_year(
        NaiveDate::from_ymd_opt(year, month, day)
            .ok_or("Invalid all-day date or leap day")?,
    )
}

impl AllDaySpan {
    pub(super) fn new(start: NaiveDate, end_exclusive: NaiveDate) -> Result<Self, String> {
        let start = valid_year(start)?;
        // End-exclusive 2100-01-01 is allowed for Dec 31, 2099.
        if end_exclusive.year() > 2100 {
            return Err("All-day end date is outside supported calendar range".into());
        }
        let days = end_exclusive.signed_duration_since(start).num_days();
        if !(1..=366).contains(&days) {
            return Err("All-day DTEND must be exclusive and 1–366 days after DTSTART".into());
        }
        Ok(Self {
            start_date: start.format("%Y-%m-%d").to_string(),
            end_date_exclusive: end_exclusive.format("%Y-%m-%d").to_string(),
        })
    }

    pub(super) fn valid(&self) -> bool {
        let Ok(start) = parse_iso_date(&self.start_date) else {
            return false;
        };
        let Ok(end) = NaiveDate::parse_from_str(&self.end_date_exclusive, "%Y-%m-%d") else {
            return false;
        };
        Self::new(start, end).is_ok()
    }

    pub(super) fn utc_midnight_ms(&self) -> Result<u64, String> {
        let date = parse_iso_date(&self.start_date)?;
        u64::try_from(
            date.and_hms_opt(0, 0, 0)
                .ok_or("Invalid midnight")?
                .and_utc()
                .timestamp_millis(),
        )
        .map_err(|_| "All-day timestamp is outside supported UTC range".into())
    }

    pub(super) fn shift_days(&self, days: i64) -> Result<Self, String> {
        let start = parse_iso_date(&self.start_date)?;
        let end = NaiveDate::parse_from_str(&self.end_date_exclusive, "%Y-%m-%d")
            .map_err(|_| "Invalid exclusive end date")?;
        let start = start
            .checked_add_signed(Duration::days(days))
            .ok_or("Recurrence start exceeded calendar range")?;
        let end = end
            .checked_add_signed(Duration::days(days))
            .ok_or("Recurrence end exceeded calendar range")?;
        Self::new(start, end)
    }

    pub(super) fn ics_date(value: &str) -> Result<String, String> {
        let date = parse_iso_date(value).or_else(|_| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| "Invalid exclusive all-day date".to_owned())
        })?;
        Ok(date.format("%Y%m%d").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_end_and_leap_day_are_date_only() {
        let span = AllDaySpan::new(
            parse_ics_date("DTSTART;VALUE=DATE", "20280229").unwrap(),
            parse_ics_date("DTEND;VALUE=DATE", "20280302").unwrap(),
        ).unwrap();
        assert_eq!(span.start_date, "2028-02-29");
        assert_eq!(span.end_date_exclusive, "2028-03-02");
        assert!(span.valid());
        assert_eq!(span.shift_days(7).unwrap().start_date, "2028-03-07");
        assert_eq!(AllDaySpan::ics_date(&span.start_date).unwrap(), "20280229");
        assert_eq!(AllDaySpan::ics_date(&span.end_date_exclusive).unwrap(), "20280302");
        assert!(parse_ics_date("DTSTART;VALUE=DATE", "20270229").is_err());
        assert!(parse_ics_date("DTSTART;TZID=Asia/Jakarta", "20280614").is_err());
    }

    #[test]
    fn invalid_ranges_are_rejected_without_datetime_assumptions() {
        let d = parse_iso_date("2027-06-14").unwrap();
        assert!(AllDaySpan::new(d, d).is_err());
        assert!(AllDaySpan::new(d, d.pred_opt().unwrap()).is_err());
        assert!(AllDaySpan::new(d, d + Duration::days(367)).is_err());
        assert!(parse_iso_date("2027-02-30").is_err());
        assert!(parse_iso_date("2027-2-3").is_err());
        assert!(parse_ics_date("DTSTART;VALUE=DATE", "20270614T000000").is_err());
    }

    #[test]
    fn midnight_utc_is_storage_anchor_not_user_display_time() {
        let span = AllDaySpan::new(
            parse_iso_date("2027-06-14").unwrap(),
            parse_iso_date("2027-06-15").unwrap(),
        ).unwrap();
        assert_eq!(span.utc_midnight_ms().unwrap(), 1_812_931_200_000);
    }
}
