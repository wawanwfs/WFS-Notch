#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use sysinfo::System;
mod calendar_ics;
mod extra;
mod parity;
mod system_details;
use tauri::{menu::MenuBuilder, tray::TrayIconBuilder};
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Settings {
    accent: String,
    hover_to_open: bool,
    pin_open: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "#d6b77a".into(),
            hover_to_open: true,
            pin_open: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Todo {
    id: String,
    title: String,
    done: bool,
    created_ms: u128,
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
    // UTC unix milliseconds; the UI displays the user's local timezone.
    starts_at_ms: u64,
    remind_minutes: Option<u32>,
    description: String,
    created_ms: u128,
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
    habits: Vec<extra::Habit>,
    quick_links: Vec<extra::QuickLink>,
    health: parity::HealthState,
    countdown: parity::CountdownState,
    settings: Settings,
    focus: FocusState,
}

impl AppData {
    fn sanitize(&mut self) {
        self.health.water_glasses = self.health.water_glasses.min(40);
        self.health.break_count = self.health.break_count.min(100);
        self.health.calm_minutes = self.health.calm_minutes.min(600);
        if !(1..=86_400).contains(&self.countdown.duration_seconds) {
            self.countdown = parity::CountdownState::default();
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
        self.events.retain(|event| {
            !event.title.trim().is_empty()
                && event.title.chars().count() <= 200
                && event.description.chars().count() <= 2000
                && (946684800000..=4102444800000).contains(&event.starts_at_ms)
                && matches!(event.remind_minutes, None | Some(0 | 5 | 15 | 30 | 60))
        });
        if ![900, 1500, 2700, 3600].contains(&self.focus.duration_seconds) {
            self.focus = FocusState::default();
        }
        self.focus.remaining_seconds = self
            .focus
            .remaining_seconds
            .min(self.focus.duration_seconds);
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
fn add_todo(store: State<'_, Mutex<Store>>, title: String) -> Result<AppData, String> {
    let title = title.trim().to_owned();
    if title.is_empty() || title.chars().count() > 200 {
        return Err("Task title must contain 1–200 characters".into());
    }
    with_store(store, |state| {
        state.todos.push(Todo {
            id: new_id(),
            title,
            done: false,
            created_ms: now_ms(),
        });
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

#[tauri::command]
fn add_event(
    store: State<'_, Mutex<Store>>,
    title: String,
    starts_at_ms: u64,
    remind_minutes: Option<u32>,
    description: String,
) -> Result<AppData, String> {
    validate_event(&title, &description, starts_at_ms, remind_minutes)?;
    with_store(store, |state| {
        state.events.push(CalendarEvent {
            id: new_id(),
            title: title.trim().to_owned(),
            starts_at_ms,
            remind_minutes,
            description,
            created_ms: now_ms(),
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
    remind_minutes: Option<u32>,
    description: String,
) -> Result<AppData, String> {
    validate_event(&title, &description, starts_at_ms, remind_minutes)?;
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
        Ok(())
    })
}

#[tauri::command]
fn remove_event(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |state| {
        let old = state.events.len();
        state.events.retain(|event| event.id != id);
        if old == state.events.len() {
            return Err("Event not found".into());
        }
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

#[tauri::command]
fn restore_backup(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    let mut locked = store
        .lock()
        .map_err(|_| "Storage lock poisoned".to_owned())?;
    let path = backup_path(&locked, &id)?;
    let mut recovered = read_state(&path)?;
    recovered.sanitize();
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
fn set_expanded(window: tauri::Window, expanded: bool) -> Result<(), String> {
    let (width, height) = if expanded {
        (760.0, 470.0)
    } else {
        (240.0, 48.0)
    };
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    if let Some(monitor) = window
        .current_monitor()
        .map_err(|error| error.to_string())?
    {
        let scale = monitor.scale_factor();
        let pixel_width = (width * scale).round() as i32;
        let position_x = monitor.position().x + (monitor.size().width as i32 - pixel_width) / 2;
        let position_y = monitor.position().y + (12.0 * scale).round() as i32;
        window
            .set_position(tauri::PhysicalPosition::new(position_x, position_y))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn close_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            fs::create_dir_all(&directory)?;
            let store = Store::open(directory.join("state.json")).map_err(std::io::Error::other)?;
            app.manage(Mutex::new(store));
            app.manage(Mutex::new(System::new_all()));
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
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let scale = monitor.scale_factor();
                    let width = (240.0 * scale).round() as i32;
                    let position_x =
                        monitor.position().x + (monitor.size().width as i32 - width) / 2;
                    let position_y = monitor.position().y + (12.0 * scale).round() as i32;
                    let _ =
                        window.set_position(tauri::PhysicalPosition::new(position_x, position_y));
                }
            }
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
            toggle_todo,
            remove_todo,
            add_note,
            update_note,
            remove_note,
            add_event,
            update_event,
            remove_event,
            calendar_ics::import_ics,
            calendar_ics::export_ics,
            create_backup,
            list_backups,
            restore_backup,
            get_system_snapshot,
            system_details::get_system_details,
            set_accent,
            set_hover_to_open,
            set_pin_open,
            set_focus_duration,
            focus_toggle,
            focus_reset,
            set_expanded,
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
            parity::health_action,
            parity::countdown_set,
            parity::countdown_toggle,
            parity::countdown_adjust,
            parity::countdown_reset
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
    fn persistence_roundtrip_and_legacy_migration() {
        let path = temp_file("roundtrip");
        fs::write(&path, r##"{"todos": [], "notes": [], "settings": {"accent":"#d6b77a", "hover_to_open": true}}"##).unwrap();
        let mut store = Store::open(path.clone()).unwrap();
        assert_eq!(store.data.focus.remaining_seconds, 1500);
        assert!(!store.data.settings.pin_open);
        store
            .transact(|state| {
                state.todos.push(Todo {
                    id: "1".into(),
                    title: "Ship features".into(),
                    done: false,
                    created_ms: now_ms(),
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
