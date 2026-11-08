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
    always_on_top: bool,
    auto_lower_fullscreen: bool,
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
            always_on_top: true,
            auto_lower_fullscreen: false,
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
}
impl Default for FocusState {
    fn default() -> Self {
        Self {
            duration_seconds: 25 * 60,
            remaining_seconds: 25 * 60,
            ends_at_ms: None,
            completed: false,
            paused: false,
        }
    }
}
impl FocusState {
    fn normalize(&mut self, current_ms: u128) {
        if let Some(end) = self.ends_at_ms {
            if end <= current_ms {
                self.remaining_seconds = 0;
                self.ends_at_ms = None;
                self.completed = true;
                self.paused = false;
            } else {
                // Ceil remaining milliseconds, including after suspend/resume.
                let remaining_ms = (end - current_ms).min(self.duration_seconds as u128 * 1000);
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
                self.remaining_seconds = self.duration_seconds;
            }
            self.ends_at_ms = Some(current_ms + self.remaining_seconds as u128 * 1000);
            self.completed = false;
            self.paused = false;
        }
    }
    fn reset(&mut self) {
        self.remaining_seconds = self.duration_seconds;
        self.ends_at_ms = None;
        self.completed = false;
        self.paused = false;
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
        if !(1..=86_400).contains(&self.countdown.duration_seconds) {
            self.countdown = wellness::CountdownState::default();
        }
        self.countdown.remaining_seconds = self.countdown.remaining_seconds.min(86_400);
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
            if todo.due_at_ms.is_some_and(|ms| !(946_684_800_000..=4_102_444_800_000).contains(&ms)) {
                todo.due_at_ms = None;
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
        self.focus.remaining_seconds = self
            .focus
            .remaining_seconds
            .min(self.focus.duration_seconds);
        self.settings.top_margin_px = self.settings.top_margin_px.min(display::MAX_MARGIN);
        if !self.settings.notifications_enabled {
            self.settings.notifications_show_details = false;
        }
        if self.settings.monitor_id != display::DEFAULT_MONITOR
            && (!self.settings.monitor_id.starts_with("origin:")
                || self.settings.monitor_id.len() > 80)
        {
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

impl Store {
    fn open(path: PathBuf) -> Result<Self, String> {
        let backup = path.with_extension("json.bak");
        let data = if path.exists() {
            match read_state(&path) {
                Ok(data) => data,
                Err(primary_error) => {
                    let restored = read_state(&backup).map_err(|backup_error| {
                        format!(
                            "Cannot recover local data: {primary_error}; backup: {backup_error}. Files were not deleted."
                        )
                    })?;
                    // Keep the invalid file for forensic recovery instead of overwriting it.
                    let damaged = path.with_extension(format!("corrupt-{}.json", now_ms()));
                    fs::rename(&path, &damaged).map_err(|error| error.to_string())?;
                    fs::copy(&backup, &path).map_err(|error| error.to_string())?;
                    restored
                }
            }
        } else if backup.exists() {
            let restored = read_state(&backup)?;
            fs::copy(&backup, &path).map_err(|error| error.to_string())?;
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
                fs::remove_file(&backup).map_err(|error| error.to_string())?;
            }
            fs::rename(&self.path, &backup).map_err(|error| error.to_string())?;
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
    let mut snapshot = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?
        .data
        .clone();
    snapshot.focus.normalize(now_ms());
    snapshot.countdown.normalize(now_ms());
    Ok(snapshot)
}

#[tauri::command]
fn add_todo(
    store: State<'_, Mutex<Store>>,
    title: String,
    priority: Option<String>,
    due_at_ms: Option<u64>,
) -> Result<AppData, String> {
    let priority = priority.unwrap_or_else(default_todo_priority);
    validate_todo_fields(&title, &priority, due_at_ms)?;
    with_store(store, |state| {
        state.todos.push(Todo {
            id: new_id(),
            title: title.trim().to_owned(),
            done: false,
            created_ms: now_ms(),
            priority,
            due_at_ms,
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
) -> Result<AppData, String> {
    validate_todo_fields(&title, &priority, due_at_ms)?;
    with_store(store, |state| {
        let todo = state.todos.iter_mut().find(|todo| todo.id == id)
            .ok_or("Task not found")?;
        todo.title = title.trim().to_owned();
        todo.priority = priority;
        todo.due_at_ms = due_at_ms;
        Ok(())
    })
}

#[tauri::command]
fn toggle_todo(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |state| {
        let todo = state
            .todos
            .iter_mut()
            .find(|todo| todo.id == id)
            .ok_or("Task not found")?;
        todo.done = !todo.done;
        Ok(())
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
) -> Result<AppData, String> {
    if title.chars().count() > 200 || body.chars().count() > 50_000 {
        return Err("Note is too long".into());
    }
    with_store(store, |state| {
        let note = state
            .notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or("Note not found")?;
        note.title = title;
        note.body = body;
        note.updated_ms = now_ms();
        Ok(())
    })
}

#[tauri::command]
fn remove_note(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |state| {
        let previous = state.notes.len();
        state.notes.retain(|note| note.id != id);
        if previous == state.notes.len() {
            return Err("Note not found".into());
        }
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

// Explicitly allow re-importing canceled ICS events, with a UI confirmation.
#[tauri::command]
fn clear_calendar_cancellations(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |data| {
        data.calendar_tombstones.clear();
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

#[tauri::command]
fn restore_backup(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    let mut locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let path = backup_path(&locked, &id)?;
    let mut recovered = read_state(&path)?;
    recovered.sanitize();
    preserve_reminder_history(&mut recovered, &locked.data);
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
            update_note,
            remove_note,
            add_event,
            update_event,
            remove_event,
            add_all_day_event,
            update_all_day_event,
            update_event_series_title,
            remove_event_series,
            clear_calendar_cancellations,
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
            notifications::notify_test,
            notifications::get_reminder_scheduler_status,
            notifications::get_windows_notification_permission,
            notifications::notify_due_event,
            set_accent,
            set_hover_to_open,
            set_pin_open,
            set_notch_on_top,
            set_fullscreen_auto,
            set_focus_duration,
            focus_toggle,
            focus_reset,
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
            wellness::countdown_reset
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
    fn task_priority_due_migration_and_validation_are_backward_compatible() {
        let old = r#"{"id":"legacy","title":"Existing task","done":false,"created_ms":1}"#;
        let todo: Todo = serde_json::from_str(old).unwrap();
        assert_eq!(todo.priority, "normal");
        assert_eq!(todo.due_at_ms, None);
        assert!(validate_todo_fields(" Ship release ", "high", Some(1_900_000_000_000)).is_ok());
        assert!(validate_todo_fields("Task", "urgent", None).is_err());
        assert!(validate_todo_fields(" ", "normal", None).is_err());
        assert!(validate_todo_fields("Task", "low", Some(1)).is_err());
        let json = serde_json::to_string(&Todo {
            priority: "high".into(),
            due_at_ms: Some(1_900_000_000_000),
            ..todo
        }).unwrap();
        let restored: Todo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.priority, "high");
        assert_eq!(restored.due_at_ms, Some(1_900_000_000_000));
    }

    #[test]
    fn task_edit_transaction_preserves_id_completion_and_rollback() {
        let path = temp_file("task-edit");
        let mut store = Store::open(path.clone()).unwrap();
        store.transact(|data| {
            data.todos.push(Todo {
                id: "task-1".into(), title: "Before".into(), done: true,
                created_ms: 123, priority: "normal".into(), due_at_ms: None,
            });
            Ok(())
        }).unwrap();
        store.transact(|data| {
            let todo = data.todos.iter_mut().find(|item| item.id == "task-1").unwrap();
            validate_todo_fields("After", "high", Some(1_900_000_000_000))?;
            todo.title = "After".into();
            todo.priority = "high".into();
            todo.due_at_ms = Some(1_900_000_000_000);
            Ok(())
        }).unwrap();
        let before = store.data.clone();
        assert!(store.transact(|state| {
            state.todos[0].title = "Would rollback".into();
            Err("simulated persistence rejection".into())
        }).is_err());
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
            });
            Err("simulated failure".into())
        });
        assert!(result.is_err());
        assert!(store.data.todos.is_empty());
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
}
