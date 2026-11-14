#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use sysinfo::System;
mod all_day;
mod calendar_ics;
mod display;
mod extra;
mod ics_time;
mod media_sessions;
mod notifications;
mod system_details;
mod wellness;
use tauri::{menu::MenuBuilder, tray::TrayIconBuilder};
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Settings {
    accent: String,
    hover_to_open: bool,
    pin_open: bool,
    monitor_id: String,
    top_margin_px: u32,
    notifications_enabled: bool,
    notifications_show_details: bool,
    notifications_paused: bool,
    // Separate explicit opt-in; existing calendar notifications do not imply task alerts.
    task_due_reminders_enabled: bool,
    always_on_top: bool,
    auto_lower_fullscreen: bool,
    // Optional local-only accessibility presets; legacy state uses defaults.
    ui_density: String,
    reduce_motion: bool,
    high_contrast: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "#d6b77a".into(),
            hover_to_open: true,
            pin_open: false,
            monitor_id: display::DEFAULT_MONITOR.into(),
            top_margin_px: 12,
            notifications_enabled: false,
            notifications_show_details: false,
            notifications_paused: false,
            task_due_reminders_enabled: false,
            always_on_top: true,
            auto_lower_fullscreen: false,
            ui_density: "comfortable".into(),
            reduce_motion: false,
            high_contrast: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Todo {
    id: String,
    title: String,
    done: bool,
    created_ms: u128,
    // Defaults preserve task records imported from v0.19 and earlier.
    #[serde(default = "default_todo_priority")]
    priority: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recurrence: Option<TaskRecurrence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recurrence_timezone: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TaskRecurrence {
    Daily,
    Weekly,
}

impl TaskRecurrence {
    fn calendar_days(self) -> i64 {
        match self {
            Self::Daily => 1,
            Self::Weekly => 7,
        }
    }

    fn next_calendar_due(self, due: u64, now: u128, zone: chrono_tz::Tz) -> Result<u64, String> {
        use chrono::{DateTime, Days, TimeZone, Utc};
        let instant = |value: u128| {
            i64::try_from(value)
                .ok()
                .and_then(DateTime::<Utc>::from_timestamp_millis)
                .ok_or("Calendar task instant outside supported range")
        };
        let anchor = instant(due.into())?.with_timezone(&zone).naive_local();
        let today = instant(now)?.with_timezone(&zone).date_naive();
        let stride = self.calendar_days();
        let skipped = (today.signed_duration_since(anchor.date()).num_days() / stride).max(1);
        // Jump directly to the current period; scan only a bounded number of
        // civil dates in case the zone skipped a date or an hour (DST gap).
        for period in skipped..skipped + 8 {
            let day = anchor
                .date()
                .checked_add_days(Days::new((period * stride) as u64))
                .ok_or("Next calendar task date is outside the supported range")?;
            let Some(next) = zone
                .from_local_datetime(&day.and_time(anchor.time()))
                .earliest()
            else {
                continue;
            };
            let next = next.timestamp_millis();
            if next > 4_102_444_800_000 {
                break;
            }
            if next > 0 && next as u128 > now && next as u64 > due {
                return Ok(next as u64);
            }
        }
        Err("No next task date in the supported calendar range".into())
    }

    fn next_due(self, due: u64, now: u128) -> Result<u64, String> {
        let step = match self {
            Self::Daily => 86_400_000_u128,
            Self::Weekly => 604_800_000_u128,
        };
        let due = u128::from(due);
        let intervals = (now.saturating_sub(due) / step).saturating_add(1);
        let next = due.saturating_add(intervals.saturating_mul(step));
        if next > 4_102_444_800_000 {
            return Err("Next task due date is outside the supported range. Turn repeat off to complete it.".into());
        }
        Ok(next as u64)
    }
}

fn task_timezone(value: &str) -> Result<chrono_tz::Tz, String> {
    if value.is_empty() || value.len() > 80 {
        return Err("Enter a valid IANA timezone, such as Asia/Jakarta".into());
    }
    value
        .parse()
        .map_err(|_| "Enter a valid IANA timezone, such as Asia/Jakarta".into())
}

#[derive(Deserialize)]
struct TaskCalendarSchedule {
    timezone: String,
    wall_time: String,
}

fn resolve_task_due(
    elapsed_due: Option<u64>,
    recurrence: Option<TaskRecurrence>,
    timezone: Option<&str>,
    wall_time: Option<&str>,
) -> Result<Option<u64>, String> {
    use chrono::{NaiveDateTime, TimeZone};
    let Some(zone) = timezone else {
        if wall_time.is_some() {
            return Err("Calendar-time tasks need an explicit timezone".into());
        }
        return Ok(elapsed_due);
    };
    if recurrence.is_none() || elapsed_due.is_some() {
        return Err(
            "Calendar-time mode requires a repeat interval and one wall-clock deadline".into(),
        );
    }
    let raw = wall_time
        .filter(|value| value.len() == 16)
        .ok_or("Enter a local date and time")?;
    let wall = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M")
        .map_err(|_| "Enter a valid calendar date and time")?;
    let due = task_timezone(zone)?
        .from_local_datetime(&wall)
        .earliest()
        .ok_or("This local time does not exist in the selected timezone. Choose another time.")?;
    u64::try_from(due.timestamp_millis())
        .map(Some)
        .map_err(|_| "Task date outside supported range".into())
}

fn validate_task_recurrence(
    due: Option<u64>,
    recurrence: Option<TaskRecurrence>,
) -> Result<(), String> {
    if recurrence.is_some() && due.is_none() {
        return Err("Repeating tasks need a due date and time".into());
    }
    Ok(())
}

fn toggle_task(todo: &mut Todo, expected_due_at_ms: Option<u64>, now: u128) -> Result<(), String> {
    if let Some(recurrence) = todo.recurrence {
        let due = todo.due_at_ms.ok_or("Repeating task has no due date")?;
        if expected_due_at_ms != Some(due) {
            return Err(
                "This task schedule changed. Review the new due date before completing it.".into(),
            );
        }
        let next = match todo.recurrence_timezone.as_deref() {
            Some(zone) => recurrence.next_calendar_due(due, now, task_timezone(zone)?)?,
            None => recurrence.next_due(due, now)?,
        };
        todo.due_at_ms = Some(next);
        todo.done = false;
    } else {
        if expected_due_at_ms.is_some() && expected_due_at_ms != todo.due_at_ms {
            return Err(
                "This task schedule changed. Review the new due date before completing it.".into(),
            );
        }
        todo.done = !todo.done;
    }
    Ok(())
}

fn default_todo_priority() -> String {
    "normal".into()
}
fn validate_todo_fields(title: &str, priority: &str, due_at_ms: Option<u64>) -> Result<(), String> {
    if title.trim().is_empty() || title.trim().chars().count() > 200 {
        return Err("Task title must contain 1–200 characters".into());
    }
    if !matches!(priority, "low" | "normal" | "high") {
        return Err("Task priority must be low, normal or high".into());
    }
    if due_at_ms.is_some_and(|ms| !(946_684_800_000..=4_102_444_800_000).contains(&ms)) {
        return Err("Task due date must be between year 2000 and 2100".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Note {
    id: String,
    title: String,
    body: String,
    updated_ms: u128,
    // Notes created before v0.24 still load unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    // Explicit optimistic-concurrency token, absent in pre-v0.28 Notes.
    #[serde(default)]
    revision: u64,
}

/// Bounded plain-text local labels; never interpret them as markup.
fn normalize_note_tags(raw: Vec<String>) -> Result<Vec<String>, String> {
    if raw.len() > 8 {
        return Err("A note supports at most 8 tags".into());
    }
    let mut tags = Vec::with_capacity(raw.len());
    for item in raw {
        let tag = item.trim().to_ascii_lowercase();
        if tag.is_empty()
            || tag.len() > 24
            || !tag
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("Each tag must be 1–24 ASCII letters, numbers, - or _".into());
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    Ok(tags)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CalendarEvent {
    id: String,
    title: String,
    // Timed event: UTC instant. All-day event: UTC-midnight storage anchor ONLY;
    // consumers must use the civil dates below, not the machine timezone.
    starts_at_ms: u64,
    remind_minutes: Option<u32>,
    description: String,
    created_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    all_day: Option<all_day::AllDaySpan>,
    // The normalized local source event ID, common to materialized occurrences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    series_uid: Option<String>,
    // Timed UTC exclusive end. Absent for legacy events and all-day spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ends_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct BackupInfo {
    id: String,
    created_ms: u128,
    todos: usize,
    notes: usize,
    events: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SystemSnapshot {
    cpu_percent: f32,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    sampled_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct FocusState {
    duration_seconds: u64,
    remaining_seconds: u64,
    ends_at_ms: Option<u128>,
    completed: bool,
    paused: bool,
    cycle_enabled: bool,
    phase: FocusPhase,
    completed_in_cycle: u8,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FocusPhase {
    #[default]
    Focus,
    ShortBreak,
    LongBreak,
}

impl Default for FocusState {
    fn default() -> Self {
        Self {
            duration_seconds: 25 * 60,
            remaining_seconds: 25 * 60,
            ends_at_ms: None,
            completed: false,
            paused: false,
            cycle_enabled: false,
            phase: FocusPhase::Focus,
            completed_in_cycle: 0,
        }
    }
}
impl FocusState {
    fn phase_seconds(&self) -> u64 {
        match self.phase {
            FocusPhase::Focus => self.duration_seconds,
            FocusPhase::ShortBreak => 5 * 60,
            FocusPhase::LongBreak => 15 * 60,
        }
    }

    fn normalize(&mut self, current_ms: u128) {
        if let Some(mut end) = self.ends_at_ms {
            if self.cycle_enabled && self.phase == FocusPhase::Focus && end <= current_ms {
                self.completed_in_cycle = self.completed_in_cycle.saturating_add(1).min(4);
                self.phase = if self.completed_in_cycle == 4 {
                    FocusPhase::LongBreak
                } else {
                    FocusPhase::ShortBreak
                };
                // Anchor the break to the original focus deadline. A long
                // sleep must not fabricate additional focus sessions.
                end = end.saturating_add(self.phase_seconds() as u128 * 1000);
                self.ends_at_ms = Some(end);
            }
            if end <= current_ms {
                if self.cycle_enabled {
                    if self.phase == FocusPhase::LongBreak {
                        self.completed_in_cycle = 0;
                    }
                    self.phase = FocusPhase::Focus;
                    self.reset();
                    return;
                }
                self.remaining_seconds = 0;
                self.ends_at_ms = None;
                self.completed = true;
                self.paused = false;
            } else {
                // Ceil remaining milliseconds, including after suspend/resume.
                let remaining_ms = (end - current_ms).min(self.phase_seconds() as u128 * 1000);
                self.remaining_seconds = remaining_ms.div_ceil(1000) as u64;
            }
        }
    }
    fn toggle(&mut self, current_ms: u128) {
        self.normalize(current_ms);
        if self.ends_at_ms.is_some() {
            self.ends_at_ms = None;
            self.paused = true;
        } else {
            if self.remaining_seconds == 0 {
                self.remaining_seconds = self.phase_seconds();
            }
            self.ends_at_ms = Some(current_ms + self.remaining_seconds as u128 * 1000);
            self.completed = false;
            self.paused = false;
        }
    }
    fn reset(&mut self) {
        self.remaining_seconds = self.phase_seconds();
        self.ends_at_ms = None;
        self.completed = false;
        self.paused = false;
    }

    fn set_cycle(&mut self, enabled: bool) {
        if self.cycle_enabled != enabled {
            self.cycle_enabled = enabled;
            self.phase = FocusPhase::Focus;
            self.completed_in_cycle = 0;
            self.reset();
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct AppData {
    todos: Vec<Todo>,
    notes: Vec<Note>,
    events: Vec<CalendarEvent>,
    // Local, title-free, persistent reservation ledger. Kept in state.json atomically.
    reminder_deliveries: HashMap<String, u128>,
    // Failed OS enqueue attempts use a bounded persistent backoff journal.
    // No event titles or notification bodies are stored in this map.
    reminder_retries: HashMap<String, notifications::ReminderRetry>,
    // Local-only ICS cancellation markers; reimport never silently restores deleted events.
    calendar_tombstones: HashSet<String>,
    // A deliberate reset starts a new history generation. An older snapshot
    // must not silently bring back markers that the user explicitly cleared.
    #[serde(skip_serializing_if = "Option::is_none")]
    calendar_cancellation_epoch: Option<String>,
    // Versioned normalized ICS source metadata. Legacy flatten-only JSON
    // deserializes to an empty map; no rules are guessed from old event data.
    calendar_series: HashMap<String, calendar_ics::SeriesSource>,
    habits: Vec<extra::Habit>,
    quick_links: Vec<extra::QuickLink>,
    health: wellness::HealthState,
    countdown: wellness::CountdownState,
    settings: Settings,
    focus: FocusState,
}

impl AppData {
    fn sanitize(&mut self) {
        self.health.water_glasses = self.health.water_glasses.min(40);
        self.health.break_count = self.health.break_count.min(100);
        self.health.calm_minutes = self.health.calm_minutes.min(600);
        self.countdown.sanitize();
        self.habits
            .retain(|habit| !habit.title.trim().is_empty() && habit.title.chars().count() <= 120);
        self.habits.truncate(200);
        self.quick_links.retain(|link| {
            !link.label.trim().is_empty()
                && link.label.chars().count() <= 120
                && (link.url.starts_with("https://") || link.url.starts_with("http://"))
                && link.url.len() <= 2048
        });
        self.quick_links.truncate(100);
        for habit in &mut self.habits {
            habit.completed_dates.retain(|date| {
                date.len() == 10 && date.bytes().all(|c| c.is_ascii_digit() || c == b'-')
            });
            habit.completed_dates.sort();
            habit.completed_dates.dedup();
            if habit.completed_dates.len() > 3660 {
                let excess = habit.completed_dates.len() - 3660;
                habit.completed_dates.drain(..excess);
            }
        }
        for todo in &mut self.todos {
            if !matches!(todo.priority.as_str(), "low" | "normal" | "high") {
                todo.priority = default_todo_priority();
            }
            if todo
                .due_at_ms
                .is_some_and(|ms| !(946_684_800_000..=4_102_444_800_000).contains(&ms))
            {
                todo.due_at_ms = None;
            }
            if todo.due_at_ms.is_none() || todo.done {
                todo.recurrence = None;
            }
            if todo
                .recurrence_timezone
                .as_deref()
                .is_some_and(|zone| task_timezone(zone).is_err())
            {
                todo.recurrence = None;
            }
            if todo.recurrence.is_none() {
                todo.recurrence_timezone = None;
            }
        }
        self.events.retain(|event| {
            !event.title.trim().is_empty()
                && event.title.chars().count() <= 200
                && event.description.chars().count() <= 2000
                && (946684800000..=4102444800000).contains(&event.starts_at_ms)
                && matches!(event.remind_minutes, None | Some(0 | 5 | 15 | 30 | 60))
                && event.all_day.as_ref().is_none_or(|span| {
                    span.valid()
                        && span.utc_midnight_ms().ok() == Some(event.starts_at_ms)
                        && event.remind_minutes.is_none()
                        && event.ends_at_ms.is_none()
                })
                && validate_timed_end(event.starts_at_ms, event.ends_at_ms).is_ok()
                && event.series_uid.as_ref().is_none_or(|uid| {
                    !uid.is_empty() && uid.len() <= 180 && !uid.contains(['\r', '\n'])
                })
        });
        self.calendar_tombstones.retain(|uid| {
            uid.starts_with("ics:") && uid.len() <= 240 && !uid.chars().any(char::is_control)
        });
        if self
            .calendar_cancellation_epoch
            .as_ref()
            .is_some_and(|epoch| {
                epoch.is_empty()
                    || epoch.len() > 80
                    || !epoch
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        {
            self.calendar_cancellation_epoch = None;
        }
        self.reminder_retries.retain(|key, retry| {
            !key.is_empty()
                && key.len() <= 240
                && !key.chars().any(char::is_control)
                && retry.valid()
        });
        self.calendar_series.retain(|key, series| series.valid(key));
        // Never truncate cancellation markers or source rules silently:
        // retain safe entries; new insertions are capped at mutation time.
        if ![900, 1500, 2700, 3600].contains(&self.focus.duration_seconds) {
            self.focus = FocusState::default();
        }
        if !self.focus.cycle_enabled && self.focus.phase != FocusPhase::Focus {
            self.focus.phase = FocusPhase::Focus;
            self.focus.reset();
        }
        self.focus.completed_in_cycle = if self.focus.cycle_enabled {
            self.focus.completed_in_cycle.min(4)
        } else {
            0
        };
        self.focus.remaining_seconds = self.focus.remaining_seconds.min(self.focus.phase_seconds());
        self.settings.top_margin_px = self.settings.top_margin_px.min(display::MAX_MARGIN);
        if !self.settings.notifications_enabled {
            self.settings.notifications_show_details = false;
        }
        if !matches!(self.settings.ui_density.as_str(), "comfortable" | "compact") {
            self.settings.ui_density = Settings::default().ui_density;
        }
        if !display::valid_monitor_preference(&self.settings.monitor_id) {
            self.settings.monitor_id = display::DEFAULT_MONITOR.into();
        }
        if self.settings.accent.len() != 7
            || !self.settings.accent.starts_with('#')
            || !self.settings.accent[1..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            self.settings.accent = Settings::default().accent;
        }
    }
}

struct Store {
    path: PathBuf,
    data: AppData,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        now_ms(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn read_state(path: &Path) -> Result<AppData, String> {
    // Limit malformed/hostile oversized state files from exhausting memory.
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > 32 * 1024 * 1024 {
        return Err("State file exceeds the 32 MB safety limit".into());
    }
    let content = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&content).map_err(|error| format!("Invalid state file: {error}"))
}

/// Find the newest readable local recovery generation without mutating any files.
/// Ignore an uncommitted .tmp snapshot: a crash must not apply a partial write.
fn read_recovery_state(backup: &Path, previous: &Path) -> Result<(AppData, PathBuf), String> {
    match read_state(backup) {
        Ok(data) => Ok((data, backup.to_path_buf())),
        Err(latest_error) => read_state(previous)
            .map(|data| (data, previous.to_path_buf()))
            .map_err(|older_error| {
                format!(
                    "Local backup recovery failed: latest: {latest_error}; older: {older_error}"
                )
            }),
    }
}

impl Store {
    fn open(path: PathBuf) -> Result<Self, String> {
        let backup = path.with_extension("json.bak");
        let previous = path.with_extension("json.bak.prev");
        let data = if path.exists() {
            match read_state(&path) {
                Ok(data) => data,
                Err(primary_error) => {
                    let (restored, recovery_path) =
                        read_recovery_state(&backup, &previous).map_err(|backup_error| {
                            format!(
                                "Cannot recover local data: {primary_error}; {backup_error}. Files were not deleted."
                            )
                        })?;
                    // Never overwrite an invalid primary in place; leave the file
                    // intact under a distinct name for manual investigation.
                    let damaged = path.with_extension(format!("corrupt-{}.json", now_ms()));
                    fs::rename(&path, &damaged).map_err(|error| error.to_string())?;
                    fs::copy(&recovery_path, &path).map_err(|error| error.to_string())?;
                    restored
                }
            }
        } else if backup.exists() || previous.exists() {
            // A crash between primary rotation and commit may leave no primary.
            let (restored, recovery_path) = read_recovery_state(&backup, &previous)?;
            fs::copy(&recovery_path, &path).map_err(|error| error.to_string())?;
            restored
        } else {
            AppData::default()
        };
        let mut data = data;
        data.sanitize();
        Ok(Self { path, data })
    }

    fn save(&self) -> Result<(), String> {
        let content = serde_json::to_vec_pretty(&self.data).map_err(|error| error.to_string())?;
        let temp = self.path.with_extension("json.tmp");
        let backup = self.path.with_extension("json.bak");
        let previous = self.path.with_extension("json.bak.prev");
        if temp.exists() {
            fs::remove_file(&temp).map_err(|error| error.to_string())?;
        }
        let mut file = File::create(&temp).map_err(|error| error.to_string())?;
        file.write_all(&content)
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if self.path.exists() {
            if backup.exists() {
                // Preserve the previous committed backup until the new primary
                // is fully serialized/fsync'd, then rotate it to a second slot.
                // An interrupted rotation can be recovered from either slot.
                if previous.exists() {
                    fs::remove_file(&previous).map_err(|error| error.to_string())?;
                }
                fs::rename(&backup, &previous).map_err(|error| error.to_string())?;
            }
            if let Err(error) = fs::rename(&self.path, &backup) {
                // Primary is still intact if rename failed. Try to restore the
                // latest backup name without ever discarding the prior snapshot.
                if !backup.exists() && previous.exists() {
                    let _ = fs::rename(&previous, &backup);
                }
                return Err(error.to_string());
            }
        }
        if let Err(error) = fs::rename(&temp, &self.path) {
            if !self.path.exists() && backup.exists() {
                let _ = fs::copy(&backup, &self.path);
            }
            return Err(error.to_string());
        }
        Ok(())
    }

    fn transact<F>(&mut self, update: F) -> Result<AppData, String>
    where
        F: FnOnce(&mut AppData) -> Result<(), String>,
    {
        let previous = self.data.clone();
        self.data.focus.normalize(now_ms());
        self.data.countdown.normalize(now_ms());
        if let Err(error) = update(&mut self.data) {
            self.data = previous;
            return Err(error);
        }
        if let Err(error) = self.save() {
            self.data = previous;
            return Err(error);
        }
        Ok(self.data.clone())
    }
}

fn with_store<F>(store: State<'_, Mutex<Store>>, update: F) -> Result<AppData, String>
where
    F: FnOnce(&mut AppData) -> Result<(), String>,
{
    store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?
        .transact(update)
}

#[tauri::command]
fn load_data(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    let mut locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let now = now_ms();
    if locked.data.focus.cycle_enabled && locked.data.focus.ends_at_ms.is_some_and(|end| end <= now)
    {
        // Still show stored data if a transient filesystem error prevents
        // persisting the transition. The UI can retry the due phase later.
        return Ok(locked
            .transact(|_| Ok(()))
            .unwrap_or_else(|_| locked.data.clone()));
    }
    let mut snapshot = locked.data.clone();
    snapshot.focus.normalize(now);
    snapshot.countdown.normalize(now);
    Ok(snapshot)
}

#[tauri::command]
fn add_todo(
    store: State<'_, Mutex<Store>>,
    title: String,
    priority: Option<String>,
    due_at_ms: Option<u64>,
    recurrence: Option<TaskRecurrence>,
    calendar_schedule: Option<TaskCalendarSchedule>,
) -> Result<AppData, String> {
    let priority = priority.unwrap_or_else(default_todo_priority);
    let due_at_ms = resolve_task_due(
        due_at_ms,
        recurrence,
        calendar_schedule.as_ref().map(|s| s.timezone.as_str()),
        calendar_schedule.as_ref().map(|s| s.wall_time.as_str()),
    )?;
    let recurrence_timezone = calendar_schedule.map(|s| s.timezone);
    validate_todo_fields(&title, &priority, due_at_ms)?;
    validate_task_recurrence(due_at_ms, recurrence)?;
    with_store(store, |state| {
        state.todos.push(Todo {
            id: new_id(),
            title: title.trim().to_owned(),
            done: false,
            created_ms: now_ms(),
            priority,
            due_at_ms,
            recurrence,
            recurrence_timezone,
        });
        Ok(())
    })
}

#[tauri::command]
fn update_todo(
    store: State<'_, Mutex<Store>>,
    id: String,
    title: String,
    priority: String,
    due_at_ms: Option<u64>,
    recurrence: Option<TaskRecurrence>,
    calendar_schedule: Option<TaskCalendarSchedule>,
) -> Result<AppData, String> {
    let due_at_ms = resolve_task_due(
        due_at_ms,
        recurrence,
        calendar_schedule.as_ref().map(|s| s.timezone.as_str()),
        calendar_schedule.as_ref().map(|s| s.wall_time.as_str()),
    )?;
    let recurrence_timezone = calendar_schedule.map(|s| s.timezone);
    validate_todo_fields(&title, &priority, due_at_ms)?;
    validate_task_recurrence(due_at_ms, recurrence)?;
    with_store(store, |state| {
        let todo = state
            .todos
            .iter_mut()
            .find(|todo| todo.id == id)
            .ok_or("Task not found")?;
        if todo.done && recurrence.is_some() {
            return Err("Reopen the completed task before setting it to repeat".into());
        }
        todo.title = title.trim().to_owned();
        todo.priority = priority;
        todo.due_at_ms = due_at_ms;
        todo.recurrence = recurrence;
        todo.recurrence_timezone = recurrence_timezone;
        Ok(())
    })
}

#[tauri::command]
fn toggle_todo(
    store: State<'_, Mutex<Store>>,
    id: String,
    expected_due_at_ms: Option<u64>,
) -> Result<AppData, String> {
    with_store(store, |state| {
        let todo = state
            .todos
            .iter_mut()
            .find(|todo| todo.id == id)
            .ok_or("Task not found")?;
        toggle_task(todo, expected_due_at_ms, now_ms())
    })
}

#[tauri::command]
fn remove_todo(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |state| {
        let previous = state.todos.len();
        state.todos.retain(|todo| todo.id != id);
        if previous == state.todos.len() {
            return Err("Task not found".into());
        }
        Ok(())
    })
}

#[tauri::command]
fn add_note(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |state| {
        state.notes.push(Note {
            id: new_id(),
            title: "Untitled".into(),
            body: String::new(),
            updated_ms: now_ms(),
            tags: Vec::new(),
            revision: 0,
        });
        Ok(())
    })
}

fn apply_note_update(
    note: &mut Note,
    title: &str,
    body: &str,
    tags: Option<Vec<String>>,
    expected_revision: u64,
) -> Result<(), String> {
    if note.revision != expected_revision {
        return Err("NOTE_CONFLICT: this note changed since the editor loaded it".into());
    }
    if title.chars().count() > 200 || body.chars().count() > 50_000 {
        return Err("Note is too long".into());
    }
    // A missing tag list preserves legacy caller tags instead of erasing them.
    let next_tags = tags.map(normalize_note_tags).transpose()?;
    if note.title == title
        && note.body == body
        && next_tags.as_ref().is_none_or(|items| note.tags == *items)
    {
        return Ok(());
    }
    let next_revision = note
        .revision
        .checked_add(1)
        .ok_or("Note revision limit exceeded")?;
    note.title = title.to_owned();
    note.body = body.to_owned();
    if let Some(tags) = next_tags {
        note.tags = tags;
    }
    note.revision = next_revision;
    note.updated_ms = now_ms();
    Ok(())
}

#[tauri::command]
fn create_note_from_draft(
    store: State<'_, Mutex<Store>>,
    title: String,
    body: String,
    tags: Vec<String>,
) -> Result<AppData, String> {
    if title.chars().count() > 200 || body.chars().count() > 50_000 {
        return Err("Note is too long".into());
    }
    let tags = normalize_note_tags(tags)?;
    with_store(store, |state| {
        state.notes.push(Note {
            id: new_id(),
            title,
            body,
            updated_ms: now_ms(),
            tags,
            revision: 0,
        });
        Ok(())
    })
}

#[tauri::command]
fn update_note(
    store: State<'_, Mutex<Store>>,
    id: String,
    title: String,
    body: String,
    tags: Option<Vec<String>>,
    expected_revision: u64,
) -> Result<AppData, String> {
    // State write and revision check share the same Mutex and disk transaction.
    with_store(store, |state| {
        let note = state
            .notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or("Note not found")?;
        apply_note_update(note, &title, &body, tags, expected_revision)
    })
}

#[tauri::command]
fn remove_note(
    store: State<'_, Mutex<Store>>,
    id: String,
    expected_revision: u64,
) -> Result<AppData, String> {
    with_store(store, |state| {
        let index = state
            .notes
            .iter()
            .position(|note| note.id == id)
            .ok_or("Note not found")?;
        if state.notes[index].revision != expected_revision {
            return Err("NOTE_CONFLICT: note changed before deletion".into());
        }
        state.notes.remove(index);
        Ok(())
    })
}

const NOTES_TRANSFER_FORMAT: &str = "wfs-notch-notes";
const NOTES_TRANSFER_MAX_BYTES: usize = 1024 * 1024;
const NOTES_TRANSFER_MAX_COUNT: usize = 250;
const NOTES_TOTAL_MAX_COUNT: usize = 500;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesTransfer {
    format: String,
    version: u32,
    notes: Vec<Note>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct NotesImportPreview {
    incoming: usize,
    new_count: usize,
    existing_count: usize,
}

fn validate_notes_transfer(payload: &str) -> Result<Vec<Note>, String> {
    if payload.len() > NOTES_TRANSFER_MAX_BYTES {
        return Err("Notes JSON exceeds the 1 MiB limit".into());
    }
    // Note itself must stay serde-compatible with legacy application state,
    // but portable imports are a strict schema: never silently discard fields.
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| format!("Invalid Notes JSON: {error}"))?;
    let entries = value
        .get("notes")
        .and_then(serde_json::Value::as_array)
        .ok_or("Notes JSON requires a notes array")?;
    for entry in entries {
        let fields = entry
            .as_object()
            .ok_or("Notes JSON entries must be objects")?;
        if fields.keys().any(|key| {
            !matches!(
                key.as_str(),
                "id" | "title" | "body" | "updated_ms" | "tags" | "revision"
            )
        }) {
            return Err("Notes JSON contains unsupported fields".into());
        }
    }
    let imported: NotesTransfer =
        serde_json::from_value(value).map_err(|error| format!("Invalid Notes JSON: {error}"))?;
    if imported.format != NOTES_TRANSFER_FORMAT || imported.version != 1 {
        return Err("Unsupported Notes JSON format/version".into());
    }
    if imported.notes.len() > NOTES_TRANSFER_MAX_COUNT {
        return Err("A Notes import supports at most 250 entries".into());
    }
    let mut ids = HashSet::new();
    for note in &imported.notes {
        if note.id.is_empty()
            || note.id.len() > 128
            || !note
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("Notes JSON contains an invalid note ID".into());
        }
        if !ids.insert(note.id.clone()) {
            return Err("Notes JSON contains duplicate note IDs".into());
        }
        if note.title.chars().count() > 200
            || note.body.chars().count() > 50_000
            || note.revision > 1_000_000_000
            || note.updated_ms > 9_007_199_254_740_991
        {
            return Err("Notes JSON contains oversized note content or revision".into());
        }
        if normalize_note_tags(note.tags.clone())? != note.tags {
            return Err("Notes JSON tags must be canonical; no implicit rewriting".into());
        }
    }
    Ok(imported.notes)
}

fn notes_import_preview(
    existing: &[Note],
    imported: &[Note],
) -> Result<NotesImportPreview, String> {
    let known: HashSet<&str> = existing.iter().map(|note| note.id.as_str()).collect();
    let new_count = imported
        .iter()
        .filter(|note| !known.contains(note.id.as_str()))
        .count();
    if existing.len().saturating_add(new_count) > NOTES_TOTAL_MAX_COUNT {
        return Err("Notes import would exceed the 500 note safety limit".into());
    }
    Ok(NotesImportPreview {
        incoming: imported.len(),
        new_count,
        existing_count: imported.len() - new_count,
    })
}

fn import_notes_into(
    data: &mut AppData,
    imported: Vec<Note>,
) -> Result<NotesImportPreview, String> {
    let preview = notes_import_preview(&data.notes, &imported)?;
    let known: HashSet<String> = data.notes.iter().map(|note| note.id.clone()).collect();
    data.notes.extend(
        imported
            .into_iter()
            .filter(|note| !known.contains(&note.id)),
    );
    Ok(preview)
}

#[tauri::command]
fn export_notes_json(store: State<'_, Mutex<Store>>) -> Result<String, String> {
    let locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    if locked.data.notes.len() > NOTES_TRANSFER_MAX_COUNT {
        return Err("Export supports at most 250 notes per snapshot".into());
    }
    let transfer = NotesTransfer {
        format: NOTES_TRANSFER_FORMAT.into(),
        version: 1,
        notes: locked.data.notes.clone(),
    };
    let json = serde_json::to_string_pretty(&transfer).map_err(|error| error.to_string())?;
    validate_notes_transfer(&json)?;
    Ok(json)
}

#[tauri::command]
fn preview_notes_import(
    store: State<'_, Mutex<Store>>,
    payload: String,
) -> Result<NotesImportPreview, String> {
    let notes = validate_notes_transfer(&payload)?;
    let locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    notes_import_preview(&locked.data.notes, &notes)
}

#[tauri::command]
fn import_notes_json(store: State<'_, Mutex<Store>>, payload: String) -> Result<AppData, String> {
    // Validate the complete payload before opening a write transaction.
    let notes = validate_notes_transfer(&payload)?;
    // Recheck collision and quota within the same atomic transaction. Even if
    // another edit happened after UI preview, existing IDs are never changed.
    with_store(store, |state| {
        import_notes_into(state, notes)?;
        Ok(())
    })
}

// Calendar events are local only; no OS calendar is modified.
fn validate_event(
    title: &str,
    description: &str,
    starts_at_ms: u64,
    remind_minutes: Option<u32>,
) -> Result<(), String> {
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("Event title must contain 1–200 characters".into());
    }
    if description.chars().count() > 2000 {
        return Err("Event description exceeds 2000 characters".into());
    }
    if !(946684800000..=4102444800000).contains(&starts_at_ms) {
        return Err("Event date must be between 2000 and 2100".into());
    }
    if !matches!(remind_minutes, None | Some(0 | 5 | 15 | 30 | 60)) {
        return Err("Reminder must be off, 0, 5, 15, 30 or 60 minutes".into());
    }
    Ok(())
}

// Calendar time bounds apply to both local editor updates and ICS imports.
const MAX_TIMED_SPAN_MS: u64 = 366 * 24 * 60 * 60 * 1000;
fn validate_timed_end(start: u64, end: Option<u64>) -> Result<(), String> {
    if let Some(end) = end {
        if end <= start || end > 4_102_444_800_000 || end - start > MAX_TIMED_SPAN_MS {
            return Err(
                "Timed end must be exclusive, after start, and within 366 days/2100".into(),
            );
        }
    }
    Ok(())
}
fn remember_calendar_cancellation(data: &mut AppData, key: &str) -> Result<(), String> {
    if !key.starts_with("ics:") || key.len() > 240 || key.chars().any(char::is_control) {
        return Err("Invalid imported calendar cancellation identifier".into());
    }
    if !data.calendar_tombstones.contains(key) && data.calendar_tombstones.len() >= 4096 {
        return Err(
            "Calendar cancellation ledger is full; clear canceled ICS history first".into(),
        );
    }
    data.calendar_tombstones.insert(key.to_owned());
    Ok(())
}

#[tauri::command]
fn add_event(
    store: State<'_, Mutex<Store>>,
    title: String,
    starts_at_ms: u64,
    ends_at_ms: Option<u64>,
    remind_minutes: Option<u32>,
    description: String,
) -> Result<AppData, String> {
    validate_event(&title, &description, starts_at_ms, remind_minutes)?;
    validate_timed_end(starts_at_ms, ends_at_ms)?;
    with_store(store, |state| {
        state.events.push(CalendarEvent {
            id: new_id(),
            title: title.trim().to_owned(),
            starts_at_ms,
            remind_minutes,
            description,
            created_ms: now_ms(),
            all_day: None,
            series_uid: None,
            ends_at_ms,
        });
        Ok(())
    })
}

#[tauri::command]
fn update_event(
    store: State<'_, Mutex<Store>>,
    id: String,
    title: String,
    starts_at_ms: u64,
    ends_at_ms: Option<u64>,
    remind_minutes: Option<u32>,
    description: String,
) -> Result<AppData, String> {
    validate_event(&title, &description, starts_at_ms, remind_minutes)?;
    validate_timed_end(starts_at_ms, ends_at_ms)?;
    with_store(store, |state| {
        let event = state
            .events
            .iter_mut()
            .find(|event| event.id == id)
            .ok_or("Event not found")?;
        event.title = title.trim().to_owned();
        event.starts_at_ms = starts_at_ms;
        event.remind_minutes = remind_minutes;
        event.description = description;
        event.all_day = None; // An explicitly timed edit converts the occurrence to timed.
        event.ends_at_ms = ends_at_ms;
        let series = event.series_uid.clone();
        let id = event.id.clone();
        if let Some(meta) = series.and_then(|uid| state.calendar_series.get_mut(&uid)) {
            meta.modified_occurrence_ids.insert(id);
        }
        Ok(())
    })
}

#[tauri::command]
fn remove_event(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |state| {
        let item = state
            .events
            .iter()
            .find(|event| event.id == id)
            .ok_or("Event not found")?;
        let tombstone =
            (item.id.starts_with("ics:") || item.series_uid.is_some()).then(|| item.id.clone());
        if let Some(key) = tombstone {
            remember_calendar_cancellation(state, &key)?;
        }
        state.events.retain(|event| event.id != id);
        Ok(())
    })
}

// New all-day events use date-only spans, never the device's midnight timezone.
fn parse_all_day_span(
    start_date: &str,
    end_date_exclusive: &str,
) -> Result<all_day::AllDaySpan, String> {
    all_day::AllDaySpan::new(
        all_day::parse_iso_date(start_date)?,
        all_day::parse_iso_date(end_date_exclusive)?,
    )
}

#[tauri::command]
fn add_all_day_event(
    store: State<'_, Mutex<Store>>,
    title: String,
    start_date: String,
    end_date_exclusive: String,
    description: String,
) -> Result<AppData, String> {
    let span = parse_all_day_span(&start_date, &end_date_exclusive)?;
    let starts_at_ms = span.utc_midnight_ms()?;
    validate_event(&title, &description, starts_at_ms, None)?;
    with_store(store, move |data| {
        data.events.push(CalendarEvent {
            id: new_id(),
            title: title.trim().to_owned(),
            starts_at_ms,
            remind_minutes: None,
            description,
            created_ms: now_ms(),
            all_day: Some(span),
            series_uid: None,
            ends_at_ms: None,
        });
        Ok(())
    })
}

#[tauri::command]
fn update_all_day_event(
    store: State<'_, Mutex<Store>>,
    id: String,
    title: String,
    start_date: String,
    end_date_exclusive: String,
    description: String,
) -> Result<AppData, String> {
    let span = parse_all_day_span(&start_date, &end_date_exclusive)?;
    let starts_at_ms = span.utc_midnight_ms()?;
    validate_event(&title, &description, starts_at_ms, None)?;
    with_store(store, move |data| {
        let event = data
            .events
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("Event not found")?;
        event.title = title.trim().to_owned();
        event.starts_at_ms = starts_at_ms;
        event.remind_minutes = None;
        event.description = description;
        event.all_day = Some(span);
        event.ends_at_ms = None;
        let series = event.series_uid.clone();
        let id = event.id.clone();
        if let Some(meta) = series.and_then(|uid| data.calendar_series.get_mut(&uid)) {
            meta.modified_occurrence_ids.insert(id);
        }
        Ok(())
    })
}

fn event_series_id(data: &AppData, id: &str) -> Result<String, String> {
    let event = data
        .events
        .iter()
        .find(|entry| entry.id == id)
        .ok_or("Calendar event not found")?;
    event
        .series_uid
        .clone()
        .ok_or("This is not an imported recurring series".into())
}

// Change only safe shared metadata; never silently shift source instants or
// rewrite the RRULE. Editing one occurrence retains its distinct event ID.
fn update_series_title(data: &mut AppData, id: &str, title: &str) -> Result<(), String> {
    let series = event_series_id(data, id)?;
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("Series title must contain 1–200 characters".into());
    }
    for event in data
        .events
        .iter_mut()
        .filter(|entry| entry.series_uid.as_deref() == Some(series.as_str()))
    {
        event.title = title.trim().to_owned();
    }
    if let Some(meta) = data.calendar_series.get_mut(&series) {
        meta.title = title.trim().to_owned();
    }
    Ok(())
}

fn remove_series(data: &mut AppData, id: &str) -> Result<(), String> {
    let series = event_series_id(data, id)?;
    remember_calendar_cancellation(data, &series)?;
    data.events
        .retain(|event| event.series_uid.as_deref() != Some(series.as_str()));
    data.calendar_series.remove(&series);
    Ok(())
}

#[tauri::command]
fn update_event_series_title(
    store: State<'_, Mutex<Store>>,
    id: String,
    title: String,
) -> Result<AppData, String> {
    with_store(store, move |data| update_series_title(data, &id, &title))
}

#[tauri::command]
fn remove_event_series(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, move |data| remove_series(data, &id))
}

/// A cautious one-time local series shift. Only true date-only or explicitly
/// UTC-scheduled source series qualify; IANA/floating timestamp reflow requires
/// DST-aware RECURRENCE-ID semantics and remains deliberately unsupported.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SeriesShiftPreview {
    occurrence_count: usize,
    delta_days: i64,
    first_old_ms: u64,
    first_new_ms: u64,
    last_old_ms: u64,
    last_new_ms: u64,
    reminders_affected: usize,
    same_start_conflicts: usize,
    fingerprint: String,
    all_day: bool,
}

fn shifted_calendar_millis(old: u64, days: i64) -> Result<u64, String> {
    let moved = i128::from(old) + i128::from(days) * 86_400_000_i128;
    let updated = u64::try_from(moved).map_err(|_| "Series shift exceeds timestamp range")?;
    if !(946_684_800_000..=4_102_444_800_000).contains(&updated) {
        return Err("Series shift is outside supported calendar years".into());
    }
    Ok(updated)
}

fn plan_series_shift(
    state: &AppData,
    event_id: &str,
    days: i64,
) -> Result<(SeriesShiftPreview, Vec<CalendarEvent>), String> {
    use std::hash::{Hash, Hasher};
    if days == 0 || !(-365..=365).contains(&days) {
        return Err("Shift must be 1–365 whole days forward or backward".into());
    }
    let uid = event_series_id(state, event_id)?;
    if state.calendar_tombstones.contains(&uid) {
        return Err("A canceled whole series cannot be rescheduled".into());
    }
    let source = state
        .calendar_series
        .get(&uid)
        .ok_or("Legacy series source unavailable; schedule shift refused")?;
    if !source.valid(&uid) || !source.modified_occurrence_ids.is_empty() {
        return Err("An edited or invalid series cannot be shifted safely as a whole".into());
    }
    let all_day = source.dtstart.0.eq_ignore_ascii_case("DTSTART;VALUE=DATE");
    if !all_day
        && !(source.dtstart.0.eq_ignore_ascii_case("DTSTART") && source.dtstart.1.ends_with('Z'))
    {
        return Err("Only all-day or explicitly UTC series can shift safely; TZID/floating DST reflow is unsupported".into());
    }
    let mut members = state
        .events
        .iter()
        .filter(|event| event.series_uid.as_deref() == Some(uid.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if members.is_empty() || members.len() > 250 {
        return Err("Series must contain 1–250 saved local occurrences".into());
    }
    members.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen_ids = HashSet::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    uid.hash(&mut hasher);
    days.hash(&mut hasher);
    source.dtstart.hash(&mut hasher);
    source.dtend.hash(&mut hasher);
    source.duration.hash(&mut hasher);
    source.rrule.hash(&mut hasher);
    source.exdates.hash(&mut hasher);
    source.rdates.hash(&mut hasher);
    // A preview also reports exact-start conflicts with OTHER events.
    // Include these in the guard: a newly added/removed/re-dated unrelated
    // event makes the old collision report stale and forces a fresh preview.
    let mut outsiders = state
        .events
        .iter()
        .filter(|event| event.series_uid.as_deref() != Some(uid.as_str()))
        .collect::<Vec<_>>();
    outsiders.sort_by(|a, b| a.id.cmp(&b.id));
    for other in outsiders {
        other.id.hash(&mut hasher);
        other.starts_at_ms.hash(&mut hasher);
    }
    let mut tombstones = state.calendar_tombstones.iter().collect::<Vec<_>>();
    tombstones.sort();
    for key in tombstones {
        key.hash(&mut hasher);
    }
    let mut moved = Vec::with_capacity(members.len());
    let mut same_start_conflicts = 0_usize;
    for existing in &members {
        if !seen_ids.insert(existing.id.clone()) || existing.all_day.is_some() != all_day {
            return Err("Mixed or duplicate occurrence identities; no shift performed".into());
        }
        existing.id.hash(&mut hasher);
        existing.starts_at_ms.hash(&mut hasher);
        existing.ends_at_ms.hash(&mut hasher);
        existing.remind_minutes.hash(&mut hasher);
        existing.title.hash(&mut hasher);
        existing.description.hash(&mut hasher);
        if let Some(span) = existing.all_day.as_ref() {
            span.start_date.hash(&mut hasher);
            span.end_date_exclusive.hash(&mut hasher);
        }
        let mut next = existing.clone();
        if let Some(span) = existing.all_day.as_ref() {
            let shifted = span.shift_days(days)?;
            next.starts_at_ms = shifted.utc_midnight_ms()?;
            next.all_day = Some(shifted);
        } else {
            next.starts_at_ms = shifted_calendar_millis(existing.starts_at_ms, days)?;
            next.ends_at_ms = existing
                .ends_at_ms
                .map(|end| shifted_calendar_millis(end, days))
                .transpose()?;
            validate_timed_end(next.starts_at_ms, next.ends_at_ms)?;
        }
        validate_event(
            &next.title,
            &next.description,
            next.starts_at_ms,
            next.remind_minutes,
        )?;
        same_start_conflicts += state
            .events
            .iter()
            .filter(|other| {
                other.series_uid.as_deref() != Some(uid.as_str())
                    && other.starts_at_ms == next.starts_at_ms
            })
            .count();
        moved.push(next);
    }
    let first_old = members.iter().min_by_key(|item| item.starts_at_ms).unwrap();
    let last_old = members.iter().max_by_key(|item| item.starts_at_ms).unwrap();
    let first_new = moved.iter().min_by_key(|item| item.starts_at_ms).unwrap();
    let last_new = moved.iter().max_by_key(|item| item.starts_at_ms).unwrap();
    let preview = SeriesShiftPreview {
        occurrence_count: members.len(),
        delta_days: days,
        first_old_ms: first_old.starts_at_ms,
        first_new_ms: first_new.starts_at_ms,
        last_old_ms: last_old.starts_at_ms,
        last_new_ms: last_new.starts_at_ms,
        reminders_affected: members
            .iter()
            .filter(|item| item.remind_minutes.is_some())
            .count(),
        same_start_conflicts,
        fingerprint: format!("{:016x}", hasher.finish()),
        all_day,
    };
    Ok((preview, moved))
}

fn shift_series_after_preview(
    state: &mut AppData,
    event_id: &str,
    days: i64,
    expected_fingerprint: &str,
) -> Result<(), String> {
    let (preview, proposed) = plan_series_shift(state, event_id, days)?;
    if expected_fingerprint.len() != 16 || preview.fingerprint != expected_fingerprint {
        return Err("Calendar series changed after preview; review the new schedule again".into());
    }
    let uid = event_series_id(state, event_id)?;
    let moves = proposed
        .into_iter()
        .map(|event| (event.id.clone(), event))
        .collect::<HashMap<_, _>>();
    for event in &mut state.events {
        if event.series_uid.as_deref() == Some(uid.as_str()) {
            if let Some(moved) = moves.get(&event.id) {
                *event = moved.clone();
            }
        }
    }
    let source = state
        .calendar_series
        .get_mut(&uid)
        .ok_or("Series metadata disappeared during shift")?;
    // The original RRULE describes the imported source, not the locally
    // shifted schedule. Mark all moved instances modified so ICS export
    // strictly flattens them and NEVER re-emits a misleading master RRULE.
    source.modified_occurrence_ids.extend(moves.keys().cloned());
    Ok(())
}

#[tauri::command]
fn preview_event_series_shift(
    store: State<'_, Mutex<Store>>,
    id: String,
    delta_days: i64,
) -> Result<SeriesShiftPreview, String> {
    let guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    plan_series_shift(&guard.data, &id, delta_days).map(|(preview, _)| preview)
}

#[tauri::command]
fn shift_event_series(
    store: State<'_, Mutex<Store>>,
    id: String,
    delta_days: i64,
    expected_fingerprint: String,
) -> Result<AppData, String> {
    with_store(store, move |data| {
        shift_series_after_preview(data, &id, delta_days, &expected_fingerprint)
    })
}

// Explicitly allow re-importing canceled ICS events, with a UI confirmation.
#[tauri::command]
fn clear_calendar_cancellations(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |data| {
        calendar_ics::reset_cancellation_history(data);
        Ok(())
    })
}

// Only app-generated backup identifiers are accepted; caller paths are never evaluated.
fn backup_path(store: &Store, id: &str) -> Result<PathBuf, String> {
    let valid = id.starts_with("backup-")
        && id.len() <= 80
        && id.ends_with(".json")
        && id[7..id.len() - 5]
            .bytes()
            .all(|c| c.is_ascii_digit() || c == b'-');
    if !valid {
        return Err("Invalid backup identifier".into());
    }
    Ok(store
        .path
        .parent()
        .ok_or("Invalid data directory")?
        .join("backups")
        .join(id))
}

fn inspect_backup(path: &Path) -> Result<BackupInfo, String> {
    let snapshot = read_state(path)?;
    Ok(BackupInfo {
        id: path
            .file_name()
            .ok_or("Bad backup name")?
            .to_string_lossy()
            .into_owned(),
        created_ms: fs::metadata(path)
            .ok()
            .and_then(|info| info.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        todos: snapshot.todos.len(),
        notes: snapshot.notes.len(),
        events: snapshot.events.len(),
    })
}

#[tauri::command]
fn create_backup(store: State<'_, Mutex<Store>>) -> Result<BackupInfo, String> {
    let locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let id = format!("backup-{}-{}.json", now_ms(), new_id().replace('-', ""));
    let path = backup_path(&locked, &id)?;
    fs::create_dir_all(path.parent().ok_or("Invalid backup directory")?)
        .map_err(|e| e.to_string())?;
    let content = serde_json::to_vec_pretty(&locked.data).map_err(|e| e.to_string())?;
    // Do not overwrite existing backups, even if timestamps collide.
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(&content).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(error.to_string());
    }
    drop(file);
    inspect_backup(&path)
}

#[tauri::command]
fn list_backups(store: State<'_, Mutex<Store>>) -> Result<Vec<BackupInfo>, String> {
    let locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let dir = locked
        .path
        .parent()
        .ok_or("Invalid data directory")?
        .join("backups");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut backups = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if backup_path(&locked, &name).is_ok()
            && entry.file_type().map_err(|e| e.to_string())?.is_file()
        {
            // Ignore damaged backup copies rather than making the whole list unavailable.
            if let Ok(backup) = inspect_backup(&entry.path()) {
                backups.push(backup);
            }
        }
    }
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_ms));
    backups.truncate(100);
    Ok(backups)
}

/// Rewinding user data must not rewind the notification-delivery history.
/// The current local journal is authoritative for at-most-once attempts, while
/// older backup entries may add history but never erase it.
fn preserve_reminder_history(recovered: &mut AppData, existing: &AppData) {
    for (key, until) in &existing.reminder_deliveries {
        recovered
            .reminder_deliveries
            .entry(key.clone())
            .and_modify(|entry| *entry = (*entry).max(*until))
            .or_insert(*until);
    }
    // A backup must not roll back sender failure history and trigger an
    // uncontrolled immediate replay on next launch. Preserve the more
    // conservative attempt count and retry eligibility timestamp.
    for (key, retry) in &existing.reminder_retries {
        recovered
            .reminder_retries
            .entry(key.clone())
            .and_modify(|entry| {
                entry.attempts = entry.attempts.max(retry.attempts);
                entry.retry_after_ms = entry.retry_after_ms.max(retry.retry_after_ms);
                entry.expires_at_ms = entry.expires_at_ms.max(retry.expires_at_ms);
            })
            .or_insert_with(|| retry.clone());
    }
}

/// A restore must not make a stale editor revision appear current again.
/// IDs that exist both before and after restore receive a fresh higher revision;
/// unchanged or deleted IDs in other snapshots are never rewritten here.
fn preserve_note_revisions(recovered: &mut AppData, existing: &AppData) -> Result<(), String> {
    let revisions: HashMap<&str, u64> = existing
        .notes
        .iter()
        .map(|note| (note.id.as_str(), note.revision))
        .collect();
    for note in &mut recovered.notes {
        if let Some(current_revision) = revisions.get(note.id.as_str()) {
            note.revision = note
                .revision
                .max(*current_revision)
                .checked_add(1)
                .ok_or("Cannot restore: note revision limit exceeded")?;
        }
    }
    Ok(())
}

#[tauri::command]
fn restore_backup(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    let mut locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let path = backup_path(&locked, &id)?;
    let mut recovered = read_state(&path)?;
    recovered.sanitize();
    preserve_reminder_history(&mut recovered, &locked.data);
    calendar_ics::preserve_cancellation_history(&mut recovered, &locked.data)?;
    preserve_note_revisions(&mut recovered, &locked.data)?;
    // Restore is transactional and retains the previous state as state.json.bak.
    locked.transact(|state| {
        *state = recovered;
        Ok(())
    })
}

#[tauri::command]
fn get_system_snapshot(system: State<'_, Mutex<System>>) -> Result<SystemSnapshot, String> {
    let mut system = system
        .lock()
        .map_err(|_| "Telemetry lock poisoned".to_owned())?;
    system.refresh_cpu_usage();
    system.refresh_memory();
    Ok(SystemSnapshot {
        cpu_percent: system.global_cpu_usage().clamp(0.0, 100.0),
        memory_used_bytes: system.used_memory(),
        memory_total_bytes: system.total_memory(),
        sampled_at_ms: now_ms(),
    })
}

#[tauri::command]
fn set_accent(store: State<'_, Mutex<Store>>, accent: String) -> Result<AppData, String> {
    if accent.len() != 7
        || !accent.starts_with('#')
        || !accent[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Accent must be #RRGGBB".into());
    }
    with_store(store, |state| {
        state.settings.accent = accent;
        Ok(())
    })
}

#[tauri::command]
fn set_appearance(
    store: State<'_, Mutex<Store>>,
    density: String,
    reduce_motion: bool,
    high_contrast: bool,
) -> Result<AppData, String> {
    if !matches!(density.as_str(), "comfortable" | "compact") {
        return Err("Invalid appearance density".into());
    }
    with_store(store, |state| {
        state.settings.ui_density = density;
        state.settings.reduce_motion = reduce_motion;
        state.settings.high_contrast = high_contrast;
        Ok(())
    })
}

#[tauri::command]
fn set_hover_to_open(store: State<'_, Mutex<Store>>, enabled: bool) -> Result<AppData, String> {
    with_store(store, |state| {
        state.settings.hover_to_open = enabled;
        Ok(())
    })
}

#[tauri::command]
fn set_pin_open(store: State<'_, Mutex<Store>>, enabled: bool) -> Result<AppData, String> {
    with_store(store, |state| {
        state.settings.pin_open = enabled;
        Ok(())
    })
}

#[tauri::command]
fn set_notch_on_top(
    window: tauri::WebviewWindow,
    store: State<'_, Mutex<Store>>,
    enabled: bool,
) -> Result<AppData, String> {
    let previous = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?
        .data
        .settings
        .always_on_top;
    let settings = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?
        .data
        .settings
        .clone();
    let mut updated = settings;
    updated.always_on_top = enabled;
    display::sync_topmost(&window, &updated)?;
    match with_store(store, |data| {
        data.settings.always_on_top = enabled;
        Ok(())
    }) {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let _ = window.set_always_on_top(previous);
            Err(error)
        }
    }
}

#[tauri::command]
fn set_focus_duration(store: State<'_, Mutex<Store>>, minutes: u64) -> Result<AppData, String> {
    if ![15, 25, 45, 60].contains(&minutes) {
        return Err("Invalid focus preset".into());
    }
    with_store(store, |state| {
        if state.focus.phase != FocusPhase::Focus {
            return Err("Set the focus duration after your break".into());
        }
        state.focus.duration_seconds = minutes * 60;
        state.focus.reset();
        Ok(())
    })
}

#[tauri::command]
fn focus_toggle(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |state| {
        state.focus.toggle(now_ms());
        Ok(())
    })
}

#[tauri::command]
fn focus_reset(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |state| {
        state.focus.reset();
        Ok(())
    })
}

#[tauri::command]
fn set_focus_cycle(store: State<'_, Mutex<Store>>, enabled: bool) -> Result<AppData, String> {
    with_store(store, |state| {
        if state.focus.ends_at_ms.is_some() || state.focus.paused {
            return Err("Reset the active timer before changing its mode".into());
        }
        state.focus.set_cycle(enabled);
        Ok(())
    })
}

#[tauri::command]
fn advance_focus_cycle(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    let mut locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let now = now_ms();
    if locked.data.focus.cycle_enabled && locked.data.focus.ends_at_ms.is_some_and(|end| end <= now)
    {
        // Store::transact normalizes both deadlines before persisting atomically.
        locked.transact(|_| Ok(()))
    } else {
        Ok(locked.data.clone())
    }
}

#[tauri::command]
fn set_fullscreen_auto(
    window: tauri::WebviewWindow,
    store: State<'_, Mutex<Store>>,
    enabled: bool,
) -> Result<AppData, String> {
    let snapshot = with_store(store, |data| {
        data.settings.auto_lower_fullscreen = enabled;
        Ok(())
    })?;
    display::sync_topmost(&window, &snapshot.settings)?;
    Ok(snapshot)
}

#[tauri::command]
fn set_expanded(
    window: tauri::WebviewWindow,
    store: State<'_, Mutex<Store>>,
    expanded: bool,
) -> Result<(), String> {
    let settings = store
        .lock()
        .map_err(|_| "Data lock is unavailable".to_owned())?
        .data
        .settings
        .clone();
    display::place_notch(&window, &settings, expanded)
}

#[tauri::command]
fn close_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            fs::create_dir_all(&directory)?;
            let store = Store::open(directory.join("state.json")).map_err(std::io::Error::other)?;
            app.manage(Mutex::new(store));
            app.manage(Mutex::new(System::new_all()));
            notifications::start_scheduler(app.handle().clone())?;
            let tray_menu = MenuBuilder::new(app)
                .text("show", "Show WFS Notch")
                .text("quit", "Exit WFS Notch")
                .build()?;
            let tray_icon = app
                .default_window_icon()
                .ok_or_else(|| std::io::Error::other("No embedded WFS Notch icon"))?
                .clone();
            TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|handle, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => handle.exit(0),
                    _ => {}
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                let settings = app
                    .state::<Mutex<Store>>()
                    .lock()
                    .map_err(|_| std::io::Error::other("Data lock unavailable"))?
                    .data
                    .settings
                    .clone();
                display::place_notch(&window, &settings, false).map_err(std::io::Error::other)?;
                display::sync_topmost(&window, &settings).map_err(std::io::Error::other)?;
            }
            display::start_guard(app.handle().clone())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_data,
            add_todo,
            update_todo,
            toggle_todo,
            remove_todo,
            add_note,
            create_note_from_draft,
            update_note,
            remove_note,
            export_notes_json,
            preview_notes_import,
            import_notes_json,
            add_event,
            update_event,
            remove_event,
            add_all_day_event,
            update_all_day_event,
            update_event_series_title,
            remove_event_series,
            preview_event_series_shift,
            shift_event_series,
            clear_calendar_cancellations,
            calendar_ics::preview_ics_import,
            calendar_ics::import_ics,
            calendar_ics::export_ics,
            create_backup,
            list_backups,
            restore_backup,
            get_system_snapshot,
            system_details::get_system_details,
            media_sessions::get_active_media_session,
            media_sessions::get_media_session_by_source,
            media_sessions::list_media_sessions,
            media_sessions::control_media_session,
            notifications::set_notification_preferences,
            notifications::set_notifications_paused,
            notifications::set_task_due_reminders,
            notifications::notify_test,
            notifications::get_reminder_scheduler_status,
            notifications::get_windows_notification_permission,
            notifications::notify_due_event,
            set_accent,
            set_appearance,
            set_hover_to_open,
            set_pin_open,
            set_notch_on_top,
            set_fullscreen_auto,
            set_focus_duration,
            focus_toggle,
            focus_reset,
            set_focus_cycle,
            advance_focus_cycle,
            set_expanded,
            display::get_display_options,
            display::set_display_preference,
            close_app,
            extra::add_habit,
            extra::toggle_habit_day,
            extra::remove_habit,
            extra::add_quick_link,
            extra::remove_quick_link,
            extra::open_quick_link,
            extra::launch_builtin,
            extra::media_key,
            extra::get_autostart,
            extra::set_autostart,
            wellness::health_action,
            wellness::countdown_set,
            wellness::countdown_toggle,
            wellness::countdown_adjust,
            wellness::countdown_reset,
            wellness::set_countdown_presets
        ])
        .run(tauri::generate_context!())
        .expect("WFS Notch encountered a fatal error");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "wfs-notch-{}-{}-{}.json",
            name,
            std::process::id(),
            new_id()
        ))
    }

    #[test]
    fn appearance_presets_migrate_validate_and_roundtrip_with_unrelated_settings() {
        let old: Settings = serde_json::from_str(
            r##"{"accent":"#d6b77a","notifications_enabled":true,"pin_open":true}"##,
        )
        .unwrap();
        assert_eq!(old.ui_density, "comfortable");
        assert!(!old.reduce_motion);
        assert!(!old.high_contrast);
        let path = temp_file("appearance-presets");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.settings = old.clone();
                data.settings.ui_density = "compact".into();
                data.settings.reduce_motion = true;
                data.settings.high_contrast = true;
                Ok(())
            })
            .unwrap();
        let restored = Store::open(path.clone()).unwrap();
        assert_eq!(restored.data.settings.ui_density, "compact");
        assert!(restored.data.settings.reduce_motion);
        assert!(restored.data.settings.high_contrast);
        assert!(restored.data.settings.notifications_enabled);
        assert!(restored.data.settings.pin_open);
        let mut malformed = restored.data.clone();
        malformed.settings.ui_density = "<script>".into();
        malformed.sanitize();
        assert_eq!(malformed.settings.ui_density, "comfortable");
        let snapshot = restored.data.clone();
        let mut store = restored;
        assert!(store
            .transact(|data| {
                data.settings.ui_density = "invalid".into();
                Err("invalid update rejected".into())
            })
            .is_err());
        assert_eq!(store.data, snapshot);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn restoring_old_notes_advances_revisions_without_overwriting_other_data() {
        let mut current = AppData::default();
        current.notes.push(Note {
            id: "same".into(),
            title: "Current text".into(),
            body: "Later".into(),
            updated_ms: 200,
            tags: vec!["current".into()],
            revision: 6,
        });
        let mut recovered = AppData::default();
        recovered.notes.push(Note {
            id: "same".into(),
            title: "Old text".into(),
            body: "Earlier".into(),
            updated_ms: 100,
            tags: vec!["backup".into()],
            revision: 1,
        });
        recovered.notes.push(Note {
            id: "only-in-backup".into(),
            title: "Preserved".into(),
            body: "Retain".into(),
            updated_ms: 50,
            tags: vec![],
            revision: 3,
        });
        preserve_note_revisions(&mut recovered, &current).unwrap();
        assert_eq!(recovered.notes[0].revision, 7);
        assert_eq!(recovered.notes[0].title, "Old text");
        assert_eq!(recovered.notes[0].body, "Earlier");
        assert_eq!(recovered.notes[0].updated_ms, 100);
        assert_eq!(recovered.notes[0].tags, vec!["backup"]);
        assert_eq!(recovered.notes[1].revision, 3);
        let before = recovered.notes[0].clone();
        assert!(apply_note_update(&mut recovered.notes[0], "Stale", "Clobber", None, 6).is_err());
        assert_eq!(recovered.notes[0], before);
        assert!(apply_note_update(&mut recovered.notes[0], "Stale", "Clobber", None, 1).is_err());
        assert_eq!(recovered.notes[0], before);

        // Reject overflow before transaction; even badly formed old snapshots
        // cannot silently wrap the monotonic token back to zero.
        let mut overflow = recovered.clone();
        overflow.notes[0].revision = u64::MAX;
        assert!(preserve_note_revisions(&mut overflow, &current).is_err());
    }

    #[test]
    fn notes_json_import_preview_rejects_bad_data_and_remains_idempotent() {
        let original = Note {
            id: "n1".into(),
            title: "Existing".into(),
            body: "Keep original".into(),
            updated_ms: 11,
            tags: vec!["work".into()],
            revision: 3,
        };
        let other = Note {
            id: "n2".into(),
            title: "Imported".into(),
            body: "<script>preserved as plain text</script>".into(),
            updated_ms: 14,
            tags: vec!["notes".into()],
            revision: 0,
        };
        let transfer = NotesTransfer {
            format: NOTES_TRANSFER_FORMAT.into(),
            version: 1,
            notes: vec![original.clone(), other.clone()],
        };
        let json = serde_json::to_string(&transfer).unwrap();
        let parsed = validate_notes_transfer(&json).unwrap();
        let preview = notes_import_preview(std::slice::from_ref(&original), &parsed).unwrap();
        assert_eq!(
            preview,
            NotesImportPreview {
                incoming: 2,
                new_count: 1,
                existing_count: 1
            }
        );
        let path = temp_file("notes-import");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.notes.push(original.clone());
                Ok(())
            })
            .unwrap();
        store
            .transact(|data| {
                import_notes_into(data, parsed.clone())?;
                Ok(())
            })
            .unwrap();
        assert_eq!(store.data.notes, vec![original.clone(), other.clone()]);
        store
            .transact(|data| {
                let result = import_notes_into(data, parsed.clone())?;
                assert_eq!(result.new_count, 0);
                assert_eq!(result.existing_count, 2);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            Store::open(path.clone()).unwrap().data.notes,
            vec![original, other]
        );
        let mut doubled = transfer.notes.clone();
        doubled.push(doubled[0].clone());
        let bad = serde_json::to_string(&NotesTransfer {
            format: NOTES_TRANSFER_FORMAT.into(),
            version: 1,
            notes: doubled,
        })
        .unwrap();
        assert!(validate_notes_transfer(&bad).is_err());
        assert!(validate_notes_transfer("{broken").is_err());
        assert!(validate_notes_transfer(&"a".repeat(NOTES_TRANSFER_MAX_BYTES + 1)).is_err());
        let mut invalid = transfer;
        invalid.version = 2;
        assert!(validate_notes_transfer(&serde_json::to_string(&invalid).unwrap()).is_err());
        invalid.version = 1;
        invalid.notes[0].tags = vec!["CAPITAL".into()];
        assert!(validate_notes_transfer(&serde_json::to_string(&invalid).unwrap()).is_err());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
        let _ = fs::remove_file(path.with_extension("json.bak.prev"));
    }

    #[test]
    fn note_revision_migrates_detects_stale_writes_and_preserves_disk_state() {
        let old = r#"{"id":"legacy","title":"A","body":"Original","updated_ms":9,"tags":["work"]}"#;
        let mut note: Note = serde_json::from_str(old).unwrap();
        assert_eq!(note.revision, 0);
        apply_note_update(&mut note, "A", "First save", None, 0).unwrap();
        assert_eq!(note.revision, 1);
        assert_eq!(note.tags, vec!["work"]);
        let before = note.clone();
        assert!(
            apply_note_update(&mut note, "Stale edit", "overwrite", None, 0)
                .unwrap_err()
                .starts_with("NOTE_CONFLICT")
        );
        assert_eq!(note, before);
        apply_note_update(&mut note, "A", "First save", None, 1).unwrap();
        assert_eq!(note.revision, 1); // No-op does not make unrelated editors stale.
        assert!(
            apply_note_update(&mut note, "A", "Test", Some(vec!["BAD TAG".into()]), 1).is_err()
        );
        assert_eq!(note, before);
        let path = temp_file("note-revision");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.notes.push(note.clone());
                Ok(())
            })
            .unwrap();
        assert!(store
            .transact(|data| { apply_note_update(&mut data.notes[0], "Old", "Stale", None, 0) })
            .is_err());
        assert_eq!(Store::open(path.clone()).unwrap().data.notes, vec![before]);
        store
            .transact(|data| apply_note_update(&mut data.notes[0], "New", "Safe content", None, 1))
            .unwrap();
        let reopened = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.data.notes[0].revision, 2);
        assert_eq!(reopened.data.notes[0].title, "New");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
        let _ = fs::remove_file(path.with_extension("json.bak.prev"));
    }

    #[test]
    fn note_tags_migrate_validate_and_survive_atomic_persistence() {
        let legacy = r#"{"id":"old","title":"Legacy","body":"Preserve me","updated_ms":9}"#;
        let mut note: Note = serde_json::from_str(legacy).unwrap();
        assert!(note.tags.is_empty());
        let tags =
            normalize_note_tags(vec![" Release ".into(), "release".into(), "dev_ops".into()])
                .unwrap();
        assert_eq!(tags, vec!["release", "dev_ops"]);
        assert!(normalize_note_tags(vec!["contains space".into()]).is_err());
        assert!(normalize_note_tags(vec!["<script>".into()]).is_err());
        assert!(normalize_note_tags(vec!["a".repeat(25)]).is_err());
        assert!(normalize_note_tags(vec!["tag".into(); 9]).is_err());
        note.tags = tags;
        let path = temp_file("note-tags");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.notes.push(note.clone());
                Ok(())
            })
            .unwrap();
        let reopened = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.data.notes, vec![note]);
        assert!(reopened.data.notes[0].body.contains("Preserve"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn legacy_settings_do_not_enable_task_due_alerts() {
        let legacy: Settings = serde_json::from_str(r#"{"notifications_enabled":true}"#).unwrap();
        assert!(legacy.notifications_enabled);
        assert!(!legacy.task_due_reminders_enabled);
        assert!(!Settings::default().task_due_reminders_enabled);
        let encoded = serde_json::to_string(&Settings {
            task_due_reminders_enabled: true,
            ..legacy
        })
        .unwrap();
        let restored: Settings = serde_json::from_str(&encoded).unwrap();
        assert!(restored.task_due_reminders_enabled);
    }

    #[test]
    fn task_priority_due_migration_and_validation_are_backward_compatible() {
        let old = r#"{"id":"legacy","title":"Existing task","done":false,"created_ms":1}"#;
        let todo: Todo = serde_json::from_str(old).unwrap();
        assert_eq!(todo.priority, "normal");
        assert_eq!(todo.due_at_ms, None);
        assert_eq!(todo.recurrence, None);
        assert!(validate_todo_fields(" Ship release ", "high", Some(1_900_000_000_000)).is_ok());
        assert!(validate_todo_fields("Task", "urgent", None).is_err());
        assert!(validate_todo_fields(" ", "normal", None).is_err());
        assert!(validate_todo_fields("Task", "low", Some(1)).is_err());
        let json = serde_json::to_string(&Todo {
            priority: "high".into(),
            due_at_ms: Some(1_900_000_000_000),
            ..todo
        })
        .unwrap();
        let restored: Todo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.priority, "high");
        assert_eq!(restored.due_at_ms, Some(1_900_000_000_000));
    }

    #[test]
    fn recurring_task_advances_once_from_its_due_anchor_and_skips_backlog() {
        let due = 1_900_000_000_000;
        let mut todo = Todo {
            id: "repeat".into(),
            title: "Review".into(),
            priority: "high".into(),
            created_ms: 1,
            done: false,
            due_at_ms: Some(due),
            recurrence: Some(TaskRecurrence::Daily),
            recurrence_timezone: None,
        };
        let now = u128::from(due + 3 * 86_400_000);
        toggle_task(&mut todo, Some(due), now).unwrap();
        let next = due + 4 * 86_400_000;
        assert_eq!(todo.due_at_ms, Some(next));
        assert!(!todo.done);
        assert_eq!(todo.id, "repeat");
        assert_eq!(todo.priority, "high");
        assert!(toggle_task(&mut todo, Some(due), now).is_err());
        assert_eq!(todo.due_at_ms, Some(next));
        let serialized = serde_json::to_string(&todo).unwrap();
        let restored: Todo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored, todo);
        assert_eq!(
            TaskRecurrence::Weekly.next_due(due, 1).unwrap(),
            due + 604_800_000
        );
        assert!(TaskRecurrence::Daily
            .next_due(4_102_444_800_000, 1)
            .is_err());
        assert!(validate_task_recurrence(None, Some(TaskRecurrence::Daily)).is_err());
    }

    #[test]
    fn calendar_recurrence_keeps_local_hour_and_resolves_dst_explicitly() {
        use chrono::DateTime;
        let utc = |value: &str| {
            DateTime::parse_from_rfc3339(value)
                .unwrap()
                .timestamp_millis() as u64
        };
        let zone = task_timezone("America/New_York").unwrap();
        let due = utc("2027-03-13T14:00:00Z");
        assert_eq!(
            TaskRecurrence::Daily
                .next_calendar_due(due, due.into(), zone)
                .unwrap(),
            utc("2027-03-14T13:00:00Z")
        );
        let gap = utc("2027-03-13T07:30:00Z");
        assert_eq!(
            TaskRecurrence::Daily
                .next_calendar_due(gap, gap.into(), zone)
                .unwrap(),
            utc("2027-03-15T06:30:00Z")
        );
        let overlap = utc("2027-11-06T05:30:00Z");
        assert_eq!(
            TaskRecurrence::Daily
                .next_calendar_due(overlap, overlap.into(), zone)
                .unwrap(),
            utc("2027-11-07T05:30:00Z")
        );
        assert_eq!(
            TaskRecurrence::Weekly
                .next_calendar_due(due, utc("2027-03-27T13:00:00Z").into(), zone)
                .unwrap(),
            utc("2027-04-03T13:00:00Z")
        );
        let resolve = |timezone, wall| {
            resolve_task_due(
                None,
                Some(TaskRecurrence::Daily),
                Some(timezone),
                Some(wall),
            )
        };
        assert!(resolve("America/New_York", "2027-03-14T02:30").is_err());
        assert_eq!(
            resolve("America/New_York", "2027-11-07T01:30").unwrap(),
            Some(utc("2027-11-07T05:30:00Z"))
        );
        assert!(resolve("Not/AZone", "2027-11-07T01:30").is_err());
        assert_eq!(
            resolve("Asia/Jakarta", "2027-11-07T09:00").unwrap(),
            Some(utc("2027-11-07T02:00:00Z"))
        );
    }

    #[test]
    fn task_edit_transaction_preserves_id_completion_and_rollback() {
        let path = temp_file("task-edit");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.todos.push(Todo {
                    id: "task-1".into(),
                    title: "Before".into(),
                    done: true,
                    created_ms: 123,
                    priority: "normal".into(),
                    due_at_ms: None,
                    recurrence: None,
                    recurrence_timezone: None,
                });
                Ok(())
            })
            .unwrap();
        store
            .transact(|data| {
                let todo = data
                    .todos
                    .iter_mut()
                    .find(|item| item.id == "task-1")
                    .unwrap();
                validate_todo_fields("After", "high", Some(1_900_000_000_000))?;
                todo.title = "After".into();
                todo.priority = "high".into();
                todo.due_at_ms = Some(1_900_000_000_000);
                Ok(())
            })
            .unwrap();
        let before = store.data.clone();
        assert!(store
            .transact(|state| {
                state.todos[0].title = "Would rollback".into();
                Err("simulated persistence rejection".into())
            })
            .is_err());
        assert_eq!(store.data.todos, before.todos);
        let loaded = Store::open(path.clone()).unwrap();
        assert_eq!(loaded.data.todos[0].title, "After");
        assert!(loaded.data.todos[0].done);
        assert_eq!(loaded.data.todos[0].id, "task-1");
        assert_eq!(loaded.data.todos[0].created_ms, 123);
        assert_eq!(loaded.data.todos[0].priority, "high");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn persistence_roundtrip_and_legacy_migration() {
        let path = temp_file("roundtrip");
        fs::write(&path, r##"{"todos": [], "notes": [], "settings": {"accent":"#d6b77a", "hover_to_open": true}}"##).unwrap();
        let mut store = Store::open(path.clone()).unwrap();
        assert_eq!(store.data.focus.remaining_seconds, 1500);
        assert!(!store.data.settings.pin_open);
        assert_eq!(store.data.settings.monitor_id, "primary");
        assert_eq!(store.data.settings.top_margin_px, 12);
        store
            .transact(|state| {
                state.todos.push(Todo {
                    id: "1".into(),
                    title: "Ship features".into(),
                    done: false,
                    created_ms: now_ms(),
                    priority: "normal".into(),
                    due_at_ms: None,
                    recurrence: None,
                    recurrence_timezone: None,
                });
                Ok(())
            })
            .unwrap();
        let reopened = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.data.todos[0].title, "Ship features");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn rejected_transaction_does_not_modify_memory() {
        let path = temp_file("rollback");
        let mut store = Store::open(path).unwrap();
        let result = store.transact(|state| {
            state.todos.push(Todo {
                id: "bad".into(),
                title: "Nope".into(),
                done: false,
                created_ms: now_ms(),
                priority: "normal".into(),
                due_at_ms: None,
                recurrence: None,
                recurrence_timezone: None,
            });
            Err("simulated failure".into())
        });
        assert!(result.is_err());
        assert!(store.data.todos.is_empty());
    }

    #[test]
    fn two_generation_recovery_survives_corrupt_primary_and_latest_backup() {
        let path = temp_file("two-generation-recovery");
        let backup = path.with_extension("json.bak");
        let previous = path.with_extension("json.bak.prev");
        let mut store = Store::open(path.clone()).unwrap();
        for accent in ["#111111", "#222222", "#333333"] {
            store
                .transact(|state| {
                    state.settings.accent = accent.into();
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(read_state(&path).unwrap().settings.accent, "#333333");
        assert_eq!(read_state(&backup).unwrap().settings.accent, "#222222");
        assert_eq!(read_state(&previous).unwrap().settings.accent, "#111111");
        fs::write(&path, "{broken primary").unwrap();
        fs::write(&backup, "{broken latest backup").unwrap();
        let recovered = Store::open(path.clone()).unwrap();
        assert_eq!(recovered.data.settings.accent, "#111111");
        assert_eq!(read_state(&path).unwrap().settings.accent, "#111111");
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "{broken latest backup"
        );
        let prefix = path.file_stem().unwrap().to_string_lossy();
        for entry in fs::read_dir(path.parent().unwrap()).unwrap().flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("{prefix}.corrupt-"))
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&previous);
    }

    #[test]
    fn failed_staging_write_preserves_committed_primary_and_both_backups() {
        let path = temp_file("staging-failure");
        let mut store = Store::open(path.clone()).unwrap();
        for accent in ["#111111", "#222222", "#333333"] {
            store
                .transact(|state| {
                    state.settings.accent = accent.into();
                    Ok(())
                })
                .unwrap();
        }
        let backup = path.with_extension("json.bak");
        let previous = path.with_extension("json.bak.prev");
        let before = (
            fs::read(&path).unwrap(),
            fs::read(&backup).unwrap(),
            fs::read(&previous).unwrap(),
        );
        // Make the staging path a directory to force a deterministic write error
        // without changing permissions or touching production data.
        let staging = path.with_extension("json.tmp");
        fs::create_dir(&staging).unwrap();
        assert!(store
            .transact(|state| {
                state.settings.accent = "#ffffff".into();
                Ok(())
            })
            .is_err());
        assert_eq!(store.data.settings.accent, "#333333");
        assert_eq!(fs::read(&path).unwrap(), before.0);
        assert_eq!(fs::read(&backup).unwrap(), before.1);
        assert_eq!(fs::read(&previous).unwrap(), before.2);
        fs::remove_dir(&staging).unwrap();
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&previous);
    }

    #[test]
    fn corrupted_primary_can_be_recovered_from_backup() {
        let path = temp_file("recover");
        let backup = path.with_extension("json.bak");
        fs::write(&path, "{bad json").unwrap();
        fs::write(&backup, serde_json::to_vec(&AppData::default()).unwrap()).unwrap();
        assert!(Store::open(path.clone()).is_ok());
        assert!(read_state(&path).is_ok());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
    }

    #[test]
    fn unrecoverable_corruption_is_not_overwritten() {
        let path = temp_file("invalid");
        fs::write(&path, "{bad json").unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{bad json");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn event_validation_rejects_bad_input() {
        assert!(validate_event("", "", 1_800_000_000_000, None).is_err());
        assert!(validate_event("valid", "", 1_800_000_000_000, Some(7)).is_err());
        assert!(validate_event("valid", "", 1_800_000_000_000, Some(15)).is_ok());
        assert!(validate_event("valid", "", 999, None).is_err());
    }

    #[test]
    fn timed_end_validation_preserves_legacy_events_and_rejects_invalid_ranges() {
        let start = 1_800_000_000_000_u64;
        assert!(validate_timed_end(start, None).is_ok());
        assert!(validate_timed_end(start, Some(start)).is_err());
        assert!(validate_timed_end(start, Some(start - 1)).is_err());
        assert!(validate_timed_end(start, Some(start + 60_000)).is_ok());
        assert!(validate_timed_end(start, Some(start + MAX_TIMED_SPAN_MS)).is_ok());
        assert!(validate_timed_end(start, Some(start + MAX_TIMED_SPAN_MS + 1)).is_err());
        let legacy = r#"{"id":"old","title":"Old","starts_at_ms":1800000000000,"remind_minutes":null,"description":"","created_ms":1}"#;
        let event: CalendarEvent = serde_json::from_str(legacy).unwrap();
        assert!(event.ends_at_ms.is_none());
        assert!(event.all_day.is_none());
        assert!(event.series_uid.is_none());
    }

    #[test]
    fn cancellation_marker_is_bounded_private_and_persists_without_titles() {
        let mut state = AppData::default();
        remember_calendar_cancellation(&mut state, "ics:series@example.org").unwrap();
        remember_calendar_cancellation(&mut state, "ics:series@example.org").unwrap();
        assert_eq!(state.calendar_tombstones.len(), 1);
        assert!(remember_calendar_cancellation(&mut state, "local-event").is_err());
        assert!(remember_calendar_cancellation(&mut state, "ics:malicious\nbody").is_err());
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("Private appointment"));
        let restored: AppData = serde_json::from_str(&json).unwrap();
        assert!(restored
            .calendar_tombstones
            .contains("ics:series@example.org"));
    }

    #[test]
    fn legacy_calendar_json_migrates_with_optional_date_and_series_metadata() {
        let previous = r#"{"id":"legacy","title":"Old","starts_at_ms":1800000000000,"remind_minutes":15,"description":"","created_ms":1}"#;
        let event: CalendarEvent = serde_json::from_str(previous).unwrap();
        assert!(event.all_day.is_none());
        assert!(event.series_uid.is_none());
        assert_eq!(event.remind_minutes, Some(15));
        let span = parse_all_day_span("2028-02-29", "2028-03-02").unwrap();
        assert!(span.valid());
        assert!(parse_all_day_span("2027-06-14", "2027-06-14").is_err());
    }

    #[test]
    fn updating_and_removing_recurring_series_are_atomic_and_scoped() {
        let mut state = AppData::default();
        for (id, series_uid) in [
            ("ics:series:1800000000000", Some("ics:series")),
            ("ics:series:1800086400000", Some("ics:series")),
            ("standalone", None),
        ] {
            state.events.push(CalendarEvent {
                id: id.into(),
                title: "Before".into(),
                starts_at_ms: 1_800_000_000_000,
                remind_minutes: None,
                description: String::new(),
                created_ms: 0,
                all_day: None,
                series_uid: series_uid.map(str::to_owned),
                ends_at_ms: None,
            });
        }
        assert!(update_series_title(&mut state, "standalone", "No").is_err());
        assert!(remove_series(&mut state, "missing").is_err());
        update_series_title(&mut state, "ics:series:1800000000000", "After").unwrap();
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| event.title == "After")
                .count(),
            2
        );
        remove_series(&mut state, "ics:series:1800000000000").unwrap();
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.events[0].id, "standalone");
    }

    fn utc_shift_fixture() -> AppData {
        let uid = "ics:shift@example.org".to_owned();
        let first = u64::try_from(
            chrono::DateTime::parse_from_rfc3339("2027-06-14T09:00:00Z")
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap();
        let mut data = AppData::default();
        data.calendar_series.insert(
            uid.clone(),
            calendar_ics::SeriesSource {
                schema_version: 1,
                uid: uid.clone(),
                title: "Team standup".into(),
                description: "Private team note".into(),
                dtstart: ("DTSTART".into(), "20270614T090000Z".into()),
                dtend: Some(("DTEND".into(), "20270614T100000Z".into())),
                duration: None,
                rrule: Some("FREQ=DAILY;COUNT=3".into()),
                exdates: vec![],
                rdates: vec![],
                floating_timezone: None,
                reminder_minutes: Some(15),
                modified_occurrence_ids: HashSet::new(),
                overrides: Vec::new(),
                cancellations: Vec::new(),
            },
        );
        for day in 0..3_u64 {
            let start = first + day * 86_400_000;
            data.events.push(CalendarEvent {
                id: format!("{uid}:{start}"),
                title: "Team standup".into(),
                starts_at_ms: start,
                remind_minutes: Some(15),
                description: "Private team note".into(),
                created_ms: 9,
                all_day: None,
                series_uid: Some(uid.clone()),
                ends_at_ms: Some(start + 3_600_000),
            });
        }
        data
    }

    #[test]
    fn shift_series_utc_requires_matching_preview_and_preserves_local_history() {
        let mut data = utc_shift_fixture();
        let id = data.events[0].id.clone();
        let before = data.events.clone();
        data.reminder_deliveries.insert(
            format!("{}:{}:15", id, before[0].starts_at_ms),
            1_900_000_000_000,
        );
        data.events.push(CalendarEvent {
            id: "standalone".into(),
            title: "Independent".into(),
            starts_at_ms: before[0].starts_at_ms + 86_400_000,
            remind_minutes: None,
            description: String::new(),
            created_ms: 3,
            all_day: None,
            series_uid: None,
            ends_at_ms: None,
        });
        let (plan, _) = plan_series_shift(&data, &id, 1).unwrap();
        assert_eq!(plan.occurrence_count, 3);
        assert_eq!(plan.reminders_affected, 3);
        assert_eq!(plan.same_start_conflicts, 1);
        assert_eq!(plan.first_new_ms, before[0].starts_at_ms + 86_400_000);
        assert!(shift_series_after_preview(&mut data, &id, 1, "outdated").is_err());
        assert_eq!(&data.events[..3], before.as_slice());
        let mut late = data.events[3].clone();
        late.id = "late-exact-start-collision".into();
        data.events.push(late);
        assert!(shift_series_after_preview(&mut data, &id, 1, &plan.fingerprint).is_err());
        data.events.pop();
        shift_series_after_preview(&mut data, &id, 1, &plan.fingerprint).unwrap();
        for (before, after) in before.iter().zip(&data.events[..3]) {
            assert_eq!(before.id, after.id);
            assert_eq!(before.created_ms, after.created_ms);
            assert_eq!(before.title, after.title);
            assert_eq!(before.remind_minutes, after.remind_minutes);
            assert_eq!(after.starts_at_ms, before.starts_at_ms + 86_400_000);
            assert_eq!(
                after.ends_at_ms,
                before.ends_at_ms.map(|end| end + 86_400_000)
            );
        }
        assert_eq!(data.events[3].id, "standalone");
        assert_eq!(data.reminder_deliveries.len(), 1);
        let source = data.calendar_series.get("ics:shift@example.org").unwrap();
        assert_eq!(source.modified_occurrence_ids.len(), 3);
        assert!(plan_series_shift(&data, &id, 1).is_err()); // no unsafe second reflow
        let restored: AppData =
            serde_json::from_str(&serde_json::to_string(&data).unwrap()).unwrap();
        assert_eq!(restored.events, data.events);
        assert_eq!(restored.calendar_series, data.calendar_series);
    }

    #[test]
    fn shift_series_all_day_preserves_exclusive_span_and_cancellations() {
        let mut data = AppData::default();
        let uid = "ics:leap@example.org";
        let span = all_day::AllDaySpan::new(
            all_day::parse_iso_date("2028-02-29").unwrap(),
            all_day::parse_iso_date("2028-03-02").unwrap(),
        )
        .unwrap();
        let start = span.utc_midnight_ms().unwrap();
        data.events.push(CalendarEvent {
            id: format!("{uid}:{start}"),
            title: "Leap event".into(),
            starts_at_ms: start,
            remind_minutes: None,
            description: String::new(),
            created_ms: 7,
            all_day: Some(span.clone()),
            series_uid: Some(uid.into()),
            ends_at_ms: None,
        });
        data.calendar_series.insert(
            uid.into(),
            calendar_ics::SeriesSource {
                schema_version: 1,
                uid: uid.into(),
                title: "Leap event".into(),
                description: String::new(),
                dtstart: ("DTSTART;VALUE=DATE".into(), "20280229".into()),
                dtend: Some(("DTEND;VALUE=DATE".into(), "20280302".into())),
                duration: None,
                rrule: Some("FREQ=YEARLY;COUNT=1".into()),
                exdates: vec![],
                rdates: vec![],
                floating_timezone: None,
                reminder_minutes: None,
                modified_occurrence_ids: HashSet::new(),
                overrides: Vec::new(),
                cancellations: Vec::new(),
            },
        );
        let id = data.events[0].id.clone();
        data.calendar_tombstones
            .insert(format!("{uid}:deleted-instance"));
        let (plan, _) = plan_series_shift(&data, &id, 1).unwrap();
        assert!(plan.all_day);
        shift_series_after_preview(&mut data, &id, 1, &plan.fingerprint).unwrap();
        assert_eq!(data.events.len(), 1); // deleted occurrence is never regenerated
        assert_eq!(
            data.events[0].all_day.as_ref().unwrap().start_date,
            "2028-03-01"
        );
        assert_eq!(
            data.events[0].all_day.as_ref().unwrap().end_date_exclusive,
            "2028-03-03"
        );
        assert_eq!(
            data.events[0].starts_at_ms,
            span.shift_days(1).unwrap().utc_midnight_ms().unwrap()
        );
        assert!(data
            .calendar_tombstones
            .contains(&format!("{uid}:deleted-instance")));
    }

    #[test]
    fn shift_series_rejects_dsty_timezone_edits_legacy_and_invalid_bounds() {
        let mut data = utc_shift_fixture();
        let id = data.events[0].id.clone();
        assert!(plan_series_shift(&data, &id, 0).is_err());
        assert!(plan_series_shift(&data, &id, 366).is_err());
        data.calendar_series
            .get_mut("ics:shift@example.org")
            .unwrap()
            .dtstart = (
            "DTSTART;TZID=America/New_York".into(),
            "20270614T090000".into(),
        );
        assert!(plan_series_shift(&data, &id, 1).is_err());
        data.calendar_series
            .get_mut("ics:shift@example.org")
            .unwrap()
            .dtstart = ("DTSTART".into(), "20270614T090000Z".into());
        data.calendar_series
            .get_mut("ics:shift@example.org")
            .unwrap()
            .modified_occurrence_ids
            .insert(id.clone());
        assert!(plan_series_shift(&data, &id, 1).is_err());
        data.calendar_series.clear();
        assert!(plan_series_shift(&data, &id, 1).is_err());
        assert!(shifted_calendar_millis(946_684_800_000, -1).is_err());
        assert!(shifted_calendar_millis(4_102_444_800_000, 1).is_err());
    }

    #[test]
    fn backups_preserve_calendar_and_migrate_legacy_data() {
        let path = temp_file("events");
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|state| {
                state.events.push(CalendarEvent {
                    id: "event-a".into(),
                    title: "Team review".into(),
                    starts_at_ms: 1_800_000_000_000,
                    remind_minutes: Some(15),
                    description: "Plan".into(),
                    created_ms: now_ms(),
                    all_day: None,
                    series_uid: None,
                    ends_at_ms: None,
                });
                Ok(())
            })
            .unwrap();
        let reloaded = Store::open(path.clone()).unwrap();
        assert_eq!(reloaded.data.events[0].title, "Team review");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn backup_restore_cannot_replay_existing_reminder_reservations() {
        let mut current = AppData::default();
        current
            .reminder_deliveries
            .insert("event:1800000000000:5".into(), 1_802_592_000_000);
        current
            .reminder_deliveries
            .insert("event-other:1800000000000:15".into(), 1_802_592_000_000);
        let mut old_backup = AppData::default();
        old_backup
            .reminder_deliveries
            .insert("event:1800000000000:5".into(), 1_800_000_300_000);
        old_backup.events.push(CalendarEvent {
            id: "event".into(),
            title: "Private".into(),
            starts_at_ms: 1_800_000_000_000,
            remind_minutes: Some(5),
            description: String::new(),
            created_ms: 0,
            all_day: None,
            series_uid: None,
            ends_at_ms: None,
        });
        preserve_reminder_history(&mut old_backup, &current);
        assert_eq!(
            old_backup.reminder_deliveries.get("event:1800000000000:5"),
            Some(&1_802_592_000_000)
        );
        assert_eq!(old_backup.reminder_deliveries.len(), 2);
        // Restore still restores event content without exposing it to the ledger.
        assert_eq!(old_backup.events[0].title, "Private");
    }

    #[test]
    fn backup_identifier_blocks_directory_traversal() {
        let store = Store::open(temp_file("backup-id")).unwrap();
        assert!(backup_path(&store, "../secret.json").is_err());
        assert!(backup_path(&store, "backup-100-42.json").is_ok());
        assert!(backup_path(&store, "backup-abc.json").is_err());
    }

    #[test]
    fn focus_pause_resume_and_elapsed_time() {
        let mut focus = FocusState::default();
        focus.toggle(1000);
        assert_eq!(focus.ends_at_ms, Some(1_501_000));
        focus.normalize(31_001);
        assert_eq!(focus.remaining_seconds, 1470);
        focus.toggle(31_001);
        assert_eq!(focus.ends_at_ms, None);
        focus.toggle(32_000);
        focus.normalize(1_502_000);
        assert!(focus.completed);
        assert_eq!(focus.remaining_seconds, 0);
    }

    #[test]
    fn pomodoro_cycle_resumes_breaks_without_inventing_focus_sessions() {
        let mut focus = FocusState::default();
        focus.set_cycle(true);
        for completed in 1..=4 {
            focus.toggle(1_000);
            let deadline = focus.ends_at_ms.unwrap();
            focus.normalize(deadline + 60_000);
            assert_eq!(focus.completed_in_cycle, completed);
            let break_seconds = if completed == 4 { 900 } else { 300 };
            assert_eq!(focus.remaining_seconds, break_seconds - 60);
            let encoded = serde_json::to_string(&focus).unwrap();
            let mut restored: FocusState = serde_json::from_str(&encoded).unwrap();
            assert_eq!(restored, focus);
            restored.normalize(deadline.saturating_sub(60_000));
            assert_eq!(restored.remaining_seconds, break_seconds);
            restored.normalize(deadline + 86_400_000);
            assert_eq!(restored.phase, FocusPhase::Focus);
            assert_eq!(restored.ends_at_ms, None);
            assert_eq!(
                restored.completed_in_cycle,
                if completed == 4 { 0 } else { completed }
            );
            focus = restored;
        }
    }

    #[test]
    fn legacy_focus_state_remains_a_single_timer() {
        let legacy = r#"{"duration_seconds":900,"remaining_seconds":60,"paused":true}"#;
        let mut focus: FocusState = serde_json::from_str(legacy).unwrap();
        assert!(!focus.cycle_enabled);
        assert_eq!(focus.phase, FocusPhase::Focus);
        focus.toggle(1_000);
        focus.normalize(62_000);
        assert!(focus.completed);
        assert_eq!(focus.completed_in_cycle, 0);
    }
}
