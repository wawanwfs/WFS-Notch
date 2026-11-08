//! Opt-in WFS-owned Windows notifications, including a native background scheduler.
//! Never accesses notifications from other applications, OS DND, or private calendars
//! outside the local WFS Notch store. All user-facing content is redacted by default.
use crate::{now_ms, with_store, AppData, CalendarEvent, Settings, Store, Todo};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

const POLL_INTERVAL: Duration = Duration::from_secs(20);
// Resume catch-up is allowed until five minutes after the scheduled event.
// A clock jump past this grace period does not flood the user with old reminders.
const EVENT_GRACE_MS: u128 = 300_000;
// Keep the reservation well beyond the five-minute delivery window, so
// short backward clock jumps or restored calendar backups cannot replay it.
// This remains a bounded best-effort guarantee, not exactly-once visibility.
const LEDGER_RETENTION_MS: u128 = 30 * 24 * 60 * 60 * 1_000;
// Explicit retry budget for definite OS enqueue errors only. An already
// reserved enqueue never retries after a process crash before acknowledgement.
const MAX_RETRY_ATTEMPTS: u8 = 3;
const MAX_RETRY_ENTRIES: usize = 4096;
const RETRY_DELAYS_MS: [u128; 3] = [20_000, 60_000, 180_000];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ReminderRetry {
    pub attempts: u8,
    pub retry_after_ms: u128,
    pub expires_at_ms: u128,
}

impl ReminderRetry {
    pub(super) fn valid(&self) -> bool {
        (1..=MAX_RETRY_ATTEMPTS).contains(&self.attempts)
            && self.retry_after_ms <= self.expires_at_ms
            && self.expires_at_ms <= u128::from(u64::MAX)
    }
}

#[derive(Serialize)]
pub struct ReminderSchedulerStatus {
    enabled: bool,
    paused: bool,
    scheduled_count: usize,
    next_due_at_ms: Option<u64>,
    task_due_reminders_enabled: bool,
    upcoming_task_due_count: usize,
    poll_interval_seconds: u64,
    persisted_delivery_count: usize,
    pending_retry_count: usize,
    suppressed_after_failures_count: usize,
    max_sender_attempts: u8,
}

fn can_notify(settings: &Settings) -> bool {
    settings.notifications_enabled && !settings.notifications_paused
}

fn reminder_start(event: &CalendarEvent) -> Option<u128> {
    event.remind_minutes.map(|minutes| {
        u128::from(event.starts_at_ms).saturating_sub(u128::from(minutes).saturating_mul(60_000))
    })
}

fn event_due(event: &CalendarEvent, now: u128) -> bool {
    reminder_start(event).is_some_and(|due| {
        now >= due && now < u128::from(event.starts_at_ms).saturating_add(EVENT_GRACE_MS)
    })
}

fn delivery_key(event: &CalendarEvent) -> String {
    format!(
        "{}:{}:{}",
        event.id,
        event.starts_at_ms,
        event.remind_minutes.unwrap_or(0)
    )
}

fn needs_delivery(
    event: &CalendarEvent,
    settings: &Settings,
    now: u128,
    already_sent: &HashMap<String, u128>,
) -> bool {
    can_notify(settings)
        && event_due(event, now)
        && !already_sent
            .get(&delivery_key(event))
            .is_some_and(|expiry| *expiry > now)
}

fn retry_allowed_key(
    key: &str,
    now: u128,
    retries: &HashMap<String, ReminderRetry>,
) -> bool {
    retries.get(key).is_none_or(|record| {
        record.expires_at_ms <= now
            || (record.attempts < MAX_RETRY_ATTEMPTS && now >= record.retry_after_ms)
    })
}

fn retry_allowed(
    event: &CalendarEvent,
    now: u128,
    retries: &HashMap<String, ReminderRetry>,
) -> bool {
    retry_allowed_key(&delivery_key(event), now, retries)
}

fn reservation_expires_at(event: &CalendarEvent) -> u128 {
    u128::from(event.starts_at_ms).saturating_add(LEDGER_RETENTION_MS)
}

/// Deterministic scheduler planner: no OS time or WinRT dependency.
fn due_event_ids(data: &AppData, now: u128) -> Vec<String> {
    if !can_notify(&data.settings) {
        return Vec::new();
    }
    data.events
        .iter()
        .filter(|event| {
            needs_delivery(event, &data.settings, now, &data.reminder_deliveries)
                && retry_allowed(event, now, &data.reminder_retries)
        })
        .map(|event| event.id.clone())
        .take(256)
        .collect()
}

/// Local task reminders are independent from calendar alerts and opt-in twice:
/// WFS-owned system toasts must be enabled AND task due reminders opted in.
fn task_delivery_key(todo: &Todo) -> Option<String> {
    todo.due_at_ms
        .map(|due| format!("task:{}:{}", todo.id, due))
}

fn task_due(todo: &Todo, now: u128) -> bool {
    todo.due_at_ms.is_some_and(|due| {
        !todo.done
            && now >= u128::from(due)
            && now < u128::from(due).saturating_add(EVENT_GRACE_MS)
    })
}

fn needs_task_delivery(todo: &Todo, settings: &Settings, now: u128, data: &AppData) -> bool {
    let Some(key) = task_delivery_key(todo) else {
        return false;
    };
    // Ensure a bounded, private journal key for old imported task IDs too.
    can_notify(settings)
        && settings.task_due_reminders_enabled
        && !todo.id.is_empty()
        && todo.id.len() <= 140
        && !todo.id.chars().any(char::is_control)
        && task_due(todo, now)
        && !data
            .reminder_deliveries
            .get(&key)
            .is_some_and(|expiry| *expiry > now)
        && retry_allowed_key(&key, now, &data.reminder_retries)
}

fn due_task_ids(data: &AppData, now: u128) -> Vec<String> {
    if !can_notify(&data.settings) || !data.settings.task_due_reminders_enabled {
        return Vec::new();
    }
    data.todos
        .iter()
        .filter(|todo| needs_task_delivery(todo, &data.settings, now, data))
        .map(|todo| todo.id.clone())
        .take(256)
        .collect()
}

fn task_message(todo: &Todo, details: bool) -> String {
    if details {
        format!(
            "Task due: {}",
            todo.title.chars().take(120).collect::<String>()
        )
    } else {
        "A task is due. Open WFS Notch to view it.".to_owned()
    }
}

fn event_message(event: &CalendarEvent, details: bool) -> String {
    if details {
        format!(
            "Upcoming: {}",
            event.title.chars().take(120).collect::<String>()
        )
    } else {
        "You have an upcoming calendar event. Open WFS Notch to view it.".to_owned()
    }
}

/// Reports the native WinRT toast setting for the WFS app identity.
/// Desktop plugin permission_state() always returns Granted on Windows,
/// which is not proof that Windows Settings permits showing toasts.
#[derive(Serialize)]
pub struct WindowsNotificationPermission {
    status: String,
    source: &'static str,
}

fn describe_native_setting(value: i32) -> &'static str {
    match value {
        0 => "enabled",
        1 => "disabled_for_application",
        2 => "disabled_for_user",
        3 => "disabled_by_policy",
        4 => "disabled_by_manifest",
        _ => "unknown",
    }
}

#[cfg(windows)]
fn native_permission_status() -> String {
    use windows::UI::Notifications::ToastNotificationManager;
    let id = windows::core::HSTRING::from("id.wfs.notch");
    match ToastNotificationManager::CreateToastNotifierWithId(&id)
        .and_then(|notifier| notifier.Setting())
    {
        Ok(setting) => describe_native_setting(setting.0).to_owned(),
        Err(_) => "unavailable".into(), // e.g. portable/no Start shortcut
    }
}
#[cfg(not(windows))]
fn native_permission_status() -> String {
    "unavailable".into()
}

#[tauri::command]
pub fn get_windows_notification_permission() -> WindowsNotificationPermission {
    WindowsNotificationPermission {
        status: native_permission_status(),
        source: "Windows ToastNotifier.Setting; Focus Assist may still suppress toasts",
    }
}

fn show(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    if native_permission_status().starts_with("disabled_") {
        return Err("Windows notification permission is disabled".into());
    }
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| format!("Windows notification unavailable: {error}"))
}

/// Reserve a reminder durably *before* calling the platform.
/// This is at-most-once enqueue, never proof of a visible Windows toast.
/// Injected wall time + sender allow real Store transactions to be tested
/// deterministically without running a Windows notification server.
fn deliver_with<F>(store: &Mutex<Store>, id: &str, now: u128, sender: F) -> Result<bool, String>
where
    F: FnOnce(&CalendarEvent, &Settings) -> Result<(), String>,
{
    if id.is_empty() || id.len() > 160 {
        return Err("Invalid calendar reminder id".into());
    }
    // Consent and write are protected by one lock: a preference change cannot
    // race a notification already being queued on this application instance.
    let mut guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    let Some(event) = guard
        .data
        .events
        .iter()
        .find(|event| event.id == id)
        .cloned()
    else {
        return Ok(false); // Event deleted before dispatch.
    };
    let settings = guard.data.settings.clone();
    if !needs_delivery(&event, &settings, now, &guard.data.reminder_deliveries)
        || !retry_allowed(&event, now, &guard.data.reminder_retries)
    {
        return Ok(false);
    }
    let key = delivery_key(&event);
    let expires_at = reservation_expires_at(&event);
    reserve_and_send(&mut guard, key, expires_at, now, || sender(&event, &settings))
}

/// Shared durable reservation and retry journal for calendar and task alerts.
/// The OS enqueue is not proof the Windows notification was visibly displayed.
fn reserve_and_send<F>(
    guard: &mut Store,
    key: String,
    expires_at: u128,
    now: u128,
    sender: F,
) -> Result<bool, String>
where
    F: FnOnce() -> Result<(), String>,
{
    // Store::transact writes to an fsync'd temporary state and renames it.
    guard.transact(|data| {
        data.reminder_deliveries.retain(|_, until| *until > now);
        data.reminder_deliveries.insert(key.clone(), expires_at);
        Ok(())
    })?;
    if let Err(_error) = sender() {
        // The OS call returned an error. Its precise presentation state may be
        // unknowable, so retries are bounded (3 total) and marked locally.
        // Commit the unreserve + retry atomically; if this transaction fails,
        // the original durable reservation survives and blocks replay.
        guard.transact(|data| {
            data.reminder_retries
                .retain(|_, retry| retry.expires_at_ms > now);
            if !data.reminder_retries.contains_key(&key)
                && data.reminder_retries.len() >= MAX_RETRY_ENTRIES
            {
                return Err("Reminder retry journal is full; send not repeated".into());
            }
            let attempts = data.reminder_retries.get(&key).map_or(1, |previous| {
                previous.attempts.saturating_add(1).min(MAX_RETRY_ATTEMPTS)
            });
            let delay = RETRY_DELAYS_MS[usize::from(attempts - 1)];
            data.reminder_retries.insert(
                key.clone(),
                ReminderRetry {
                    attempts,
                    retry_after_ms: now.saturating_add(delay),
                    expires_at_ms: expires_at,
                },
            );
            data.reminder_deliveries.remove(&key);
            Ok(())
        })?;
        // Do not log event title, source UID, sender error body or descriptions.
        return Err("Windows rejected notification enqueue; bounded retry scheduled".into());
    }
    // Success already has a durably committed reservation. Removing a stale
    // retry marker is housekeeping, not a reason to repeat the OS send.
    if guard.data.reminder_retries.contains_key(&key) {
        let _ = guard.transact(|data| {
            data.reminder_retries.remove(&key);
            Ok(())
        });
    }
    Ok(true)
}

/// Only a currently open, still-due, explicitly opted-in task can reserve/send.
/// The task key includes its due time, so edits and retries never reuse stale IDs.
fn deliver_task_with<F>(
    store: &Mutex<Store>,
    id: &str,
    now: u128,
    sender: F,
) -> Result<bool, String>
where
    F: FnOnce(&Todo, &Settings) -> Result<(), String>,
{
    if id.is_empty() || id.len() > 140 || id.chars().any(char::is_control) {
        return Err("Invalid task reminder id".into());
    }
    let mut guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    let Some(todo) = guard.data.todos.iter().find(|todo| todo.id == id).cloned() else {
        return Ok(false);
    };
    let settings = guard.data.settings.clone();
    if !needs_task_delivery(&todo, &settings, now, &guard.data) {
        return Ok(false);
    }
    let key = task_delivery_key(&todo).ok_or("Task due date was cleared")?;
    let expires_at = u128::from(todo.due_at_ms.ok_or("Task due date was cleared")?)
        .saturating_add(LEDGER_RETENTION_MS);
    reserve_and_send(&mut guard, key, expires_at, now, || sender(&todo, &settings))
}

fn deliver_task_by_id(app: &AppHandle, store: &Mutex<Store>, id: &str) -> Result<bool, String> {
    deliver_task_with(store, id, now_ms(), |todo, settings| {
        show(
            app,
            "WFS Notch · Tasks",
            &task_message(todo, settings.notifications_show_details),
        )
    })
}

fn deliver_by_id(app: &AppHandle, store: &Mutex<Store>, id: &str) -> Result<bool, String> {
    deliver_with(store, id, now_ms(), |event, settings| {
        show(
            app,
            "WFS Notch · Calendar",
            &event_message(event, settings.notifications_show_details),
        )
    })
}

/// Single scheduler pass; no window references or JS callbacks are involved.
/// This keeps reminders active when the notch is collapsed or its WebView is hidden.
fn poll_due_events(app: &AppHandle) -> Result<usize, String> {
    let store = app.state::<Mutex<Store>>();
    let (due_ids, task_ids) = {
        let guard = store
            .lock()
            .map_err(|_| "Storage lock unavailable".to_owned())?;
        let now = now_ms();
        (due_event_ids(&guard.data, now), due_task_ids(&guard.data, now))
    };
    let mut delivered = 0;
    for id in due_ids {
        match deliver_by_id(app, store.inner(), &id) {
            Ok(true) => delivered += 1,
            Ok(false) => {}
            Err(_) => {
                // No event titles, source UIDs or Windows error strings logged.
                eprintln!("WFS reminder enqueue failed (retry bounded by local journal)");
            }
        }
    }
    for id in task_ids {
        match deliver_task_by_id(app, store.inner(), &id) {
            Ok(true) => delivered += 1,
            Ok(false) => {}
            Err(_) => {
                // Never log private task titles, identifiers or sender contents.
                eprintln!("WFS task reminder enqueue failed (bounded local retry)");
            }
        }
    }
    Ok(delivered)
}

pub fn start_scheduler(app: AppHandle) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("wfs-reminder-scheduler".into())
        .spawn(move || loop {
            if let Err(error) = poll_due_events(&app) {
                eprintln!("WFS reminder scheduler: {error}");
            }
            std::thread::sleep(POLL_INTERVAL);
        })
        .map(|_| ())
}

#[tauri::command]
pub fn get_reminder_scheduler_status(
    store: State<'_, Mutex<Store>>,
) -> Result<ReminderSchedulerStatus, String> {
    let guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    let now = now_ms();
    let scheduled = guard
        .data
        .events
        .iter()
        .filter_map(reminder_start)
        .filter(|due| *due >= now)
        .collect::<Vec<_>>();
    Ok(ReminderSchedulerStatus {
        enabled: guard.data.settings.notifications_enabled,
        paused: guard.data.settings.notifications_paused,
        scheduled_count: scheduled.len(),
        next_due_at_ms: scheduled
            .iter()
            .min()
            .and_then(|value| u64::try_from(*value).ok()),
        task_due_reminders_enabled: guard.data.settings.task_due_reminders_enabled,
        upcoming_task_due_count: if can_notify(&guard.data.settings)
            && guard.data.settings.task_due_reminders_enabled
        {
            guard
                .data
                .todos
                .iter()
                .filter(|todo| !todo.done && todo.due_at_ms.is_some_and(|due| u128::from(due) >= now))
                .count()
        } else {
            0
        },
        poll_interval_seconds: POLL_INTERVAL.as_secs(),
        persisted_delivery_count: guard
            .data
            .reminder_deliveries
            .values()
            .filter(|expiry| **expiry > now)
            .count(),
        pending_retry_count: guard
            .data
            .reminder_retries
            .values()
            .filter(|entry| entry.expires_at_ms > now && entry.attempts < MAX_RETRY_ATTEMPTS)
            .count(),
        suppressed_after_failures_count: guard
            .data
            .reminder_retries
            .values()
            .filter(|entry| entry.expires_at_ms > now && entry.attempts >= MAX_RETRY_ATTEMPTS)
            .count(),
        max_sender_attempts: MAX_RETRY_ATTEMPTS,
    })
}

#[tauri::command]
pub fn set_notification_preferences(
    store: State<'_, Mutex<Store>>,
    enabled: bool,
    show_details: bool,
) -> Result<AppData, String> {
    with_store(store, |data| {
        data.settings.notifications_enabled = enabled;
        data.settings.notifications_show_details = enabled && show_details;
        Ok(())
    })
}

#[tauri::command]
pub fn set_notifications_paused(
    store: State<'_, Mutex<Store>>,
    paused: bool,
) -> Result<AppData, String> {
    with_store(store, |data| {
        data.settings.notifications_paused = paused;
        Ok(())
    })
}

#[tauri::command]
pub fn set_task_due_reminders(
    store: State<'_, Mutex<Store>>,
    enabled: bool,
) -> Result<AppData, String> {
    with_store(store, |data| {
        data.settings.task_due_reminders_enabled = enabled;
        Ok(())
    })
}

#[tauri::command]
pub fn notify_test(app: AppHandle, store: State<'_, Mutex<Store>>) -> Result<bool, String> {
    // The lock prevents a change to preferences during the notification send.
    let guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    if !can_notify(&guard.data.settings) {
        return Ok(false);
    }
    show(
        &app,
        "WFS Notch",
        "Desktop notifications are enabled. Private information is hidden by default.",
    )?;
    Ok(true)
}

#[tauri::command]
pub fn notify_due_event(
    app: AppHandle,
    store: State<'_, Mutex<Store>>,
    id: String,
) -> Result<bool, String> {
    deliver_by_id(&app, store.inner(), &id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, due_at_ms: u64) -> Todo {
        Todo {
            id: id.into(),
            title: "Private launch".into(),
            done: false,
            created_ms: 1,
            priority: "high".into(),
            due_at_ms: Some(due_at_ms),
        }
    }

    #[test]
    fn tasks_require_separate_consent_and_skip_completed_edited_or_deleted() {
        let time = 1_900_000_000_000_u64;
        let mut data = AppData {
            todos: vec![task("task-1", time)],
            ..AppData::default()
        };
        assert!(due_task_ids(&data, u128::from(time)).is_empty());
        data.settings.notifications_enabled = true;
        assert!(due_task_ids(&data, u128::from(time)).is_empty());
        data.settings.task_due_reminders_enabled = true;
        assert_eq!(due_task_ids(&data, u128::from(time)), vec!["task-1"]);
        assert!(!task_message(&data.todos[0], false).contains("Private launch"));
        assert!(task_message(&data.todos[0], true).contains("Private launch"));
        assert!(due_task_ids(&data, u128::from(time) - 1).is_empty());
        let key = task_delivery_key(&data.todos[0]).unwrap();
        data.reminder_deliveries
            .insert(key, u128::from(time) + LEDGER_RETENTION_MS);
        assert!(due_task_ids(&data, u128::from(time)).is_empty());
        data.todos[0].title = "Edited title".into();
        assert!(due_task_ids(&data, u128::from(time)).is_empty());
        data.todos[0].due_at_ms = Some(time + 60_000);
        assert_eq!(due_task_ids(&data, u128::from(time) + 60_000), vec!["task-1"]);
        data.todos[0].done = true;
        assert!(due_task_ids(&data, u128::from(time) + 60_000).is_empty());
        data.todos.clear();
        assert!(due_task_ids(&data, u128::from(time) + 60_000).is_empty());
    }

    #[test]
    fn task_reservations_survive_restart_and_retry_without_private_details() {
        let due = 1_900_000_000_000_u64;
        let path = std::env::temp_dir().join(format!("wfs-task-alert-{}.json", crate::new_id()));
        let mut initial = Store::open(path.clone()).unwrap();
        initial
            .transact(|data| {
                data.settings.notifications_enabled = true;
                data.settings.task_due_reminders_enabled = true;
                data.todos.push(task("task-1", due));
                Ok(())
            })
            .unwrap();
        let store = Mutex::new(initial);
        let error = deliver_task_with(&store, "task-1", u128::from(due), |_, _| {
            Err("private sender failure".into())
        })
        .unwrap_err();
        assert!(!error.contains("private"));
        assert!(!error.contains("task-1"));
        assert!(!deliver_task_with(&store, "task-1", u128::from(due) + 1, |_, _| {
            panic!("Retry was scheduled in the future")
        })
        .unwrap());
        assert!(deliver_task_with(&store, "task-1", u128::from(due) + 20_000, |todo, _| {
            assert_eq!(todo.id, "task-1");
            Ok(())
        })
        .unwrap());
        let from_disk = Store::open(path.clone()).unwrap();
        let after_restart = Mutex::new(from_disk);
        assert!(!deliver_task_with(&after_restart, "task-1", u128::from(due) + 25_000, |_, _| {
            panic!("Successful durable reservation must not replay")
        })
        .unwrap());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
    }

    fn event(starts_at_ms: u64, remind_minutes: Option<u32>) -> CalendarEvent {
        CalendarEvent {
            id: "event-1".into(),
            title: "Private appointment".into(),
            starts_at_ms,
            remind_minutes,
            description: String::new(),
            created_ms: 0,
            all_day: None,
            series_uid: None,
            ends_at_ms: None,
        }
    }

    #[test]
    fn event_due_rejects_unscheduled_and_far_future() {
        assert!(!event_due(&event(1_000_000, None), 1_000_000));
        assert!(!event_due(&event(1_000_000, Some(5)), 100_000));
        assert!(event_due(&event(1_000_000, Some(5)), 700_000));
        assert!(!event_due(&event(1_000_000, Some(5)), 1_400_000));
    }

    #[test]
    fn native_scheduler_checks_due_window_after_resume() {
        let upcoming = event(1_500_000, Some(15));
        assert!(event_due(&upcoming, 620_000));
        assert!(event_due(&upcoming, 1_199_999));
        assert!(event_due(&upcoming, 1_200_000));
        assert!(event_due(&upcoming, 1_799_999));
        assert!(!event_due(&upcoming, 1_800_000));
    }

    #[test]
    fn paused_disabled_and_duplicate_messages_are_suppressed() {
        let mut settings = Settings::default();
        let upcoming = event(1_000_000, Some(5));
        let mut sent = HashMap::new();
        assert!(!needs_delivery(&upcoming, &settings, 700_000, &sent));
        settings.notifications_enabled = true;
        assert!(needs_delivery(&upcoming, &settings, 700_000, &sent));
        settings.notifications_paused = true;
        assert!(!needs_delivery(&upcoming, &settings, 700_000, &sent));
        settings.notifications_paused = false;
        sent.insert(delivery_key(&upcoming), 1_300_000);
        assert!(!needs_delivery(&upcoming, &settings, 700_000, &sent));
        let changed = event(1_100_000, Some(5));
        assert!(needs_delivery(&changed, &settings, 800_000, &sent));
        // Same key survives a restart, but an edited event gets a fresh key.
        let restored: HashMap<String, u128> =
            serde_json::from_str(&serde_json::to_string(&sent).unwrap()).unwrap();
        assert!(!needs_delivery(&upcoming, &settings, 700_000, &restored));
    }

    #[test]
    fn sensitive_title_is_hidden_by_default() {
        let e = event(1_000_000, Some(15));
        let hidden = event_message(&e, false);
        assert!(!hidden.contains("Private appointment"));
        assert!(event_message(&e, true).contains("Private appointment"));
    }

    #[test]
    fn wake_up_and_clock_changes_do_not_duplicate_deliveries() {
        let settings = Settings {
            notifications_enabled: true,
            ..Settings::default()
        };
        let appointment = event(3_600_000, Some(60));
        let due = reminder_start(&appointment).unwrap();
        let mut sent = HashMap::new();
        assert!(needs_delivery(&appointment, &settings, due, &sent));
        sent.insert(delivery_key(&appointment), 3_900_000);
        let disk = serde_json::to_string(&sent).unwrap();
        let after_restart: HashMap<String, u128> = serde_json::from_str(&disk).unwrap();
        assert!(!needs_delivery(
            &appointment,
            &settings,
            due + 20_000,
            &after_restart
        ));
        assert!(!needs_delivery(
            &appointment,
            &settings,
            due,
            &after_restart
        )); // clock moved back
        assert!(!event_due(&appointment, 3_900_000)); // stale after resume
        let changed = event(3_660_000, Some(60));
        assert!(needs_delivery(&changed, &settings, 60_000, &after_restart));
    }

    fn fixture_store(id: &str, time: u64) -> (Mutex<Store>, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("wfs-phase13-{}-{}.json", crate::new_id(), id));
        let mut store = Store::open(path.clone()).unwrap();
        store
            .transact(|data| {
                data.settings.notifications_enabled = true;
                data.events.push(event(time, Some(5)));
                Ok(())
            })
            .unwrap();
        (Mutex::new(store), path)
    }

    #[test]
    fn scheduler_planner_respects_consent_edit_delete_and_clock() {
        let settings = Settings {
            notifications_enabled: true,
            ..Settings::default()
        };
        let first = event(1_000_000, Some(5));
        let mut data = AppData {
            events: vec![first.clone()],
            settings,
            ..AppData::default()
        };
        assert!(due_event_ids(&data, 699_999).is_empty());
        assert_eq!(due_event_ids(&data, 700_000), vec!["event-1"]);
        data.settings.notifications_paused = true;
        assert!(due_event_ids(&data, 700_000).is_empty());
        data.settings.notifications_paused = false;
        data.reminder_deliveries
            .insert(delivery_key(&first), reservation_expires_at(&first));
        assert!(due_event_ids(&data, 700_000).is_empty());
        assert!(due_event_ids(&data, 800_000).is_empty()); // clock moved backward
        data.events[0].starts_at_ms = 1_100_000; // edited schedule
        assert_eq!(due_event_ids(&data, 800_000), vec!["event-1"]);
        data.events.clear();
        assert!(due_event_ids(&data, 800_000).is_empty());
    }

    #[test]
    fn durable_enqueues_survive_restart_and_retry_on_sender_error() {
        let (store, path) = fixture_store("replay", 1_800_000_000_000);
        let mut sends = 0;
        assert!(deliver_with(&store, "event-1", 1_799_999_700_000, |_, _| {
            sends += 1;
            Ok(())
        })
        .unwrap());
        assert_eq!(sends, 1);
        // Repeated polls and application restarts must not duplicate attempts.
        assert!(!deliver_with(&store, "event-1", 1_799_999_710_000, |_, _| {
            sends += 1;
            Ok(())
        })
        .unwrap());
        let reloaded = Mutex::new(Store::open(path.clone()).unwrap());
        assert!(
            !deliver_with(&reloaded, "event-1", 1_799_999_700_000, |_, _| {
                sends += 1;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(sends, 1);
        reloaded
            .lock()
            .unwrap()
            .transact(|data| {
                data.events[0].starts_at_ms = 1_800_000_100_000;
                Ok(())
            })
            .unwrap();
        assert!(
            deliver_with(&reloaded, "event-1", 1_799_999_800_000, |_, _| {
                sends += 1;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(sends, 2);
        drop(reloaded);
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn rejected_platform_send_releases_reservation_without_logging_titles() {
        let (store, path) = fixture_store("failure", 1_800_000_000_000);
        assert!(deliver_with(&store, "event-1", 1_799_999_700_000, |_, _| {
            Err("OS notification disabled".into())
        })
        .is_err());
        assert!(store.lock().unwrap().data.reminder_deliveries.is_empty());
        assert_eq!(store.lock().unwrap().data.reminder_retries.len(), 1);
        assert!(!deliver_with(&store, "event-1", 1_799_999_700_100, |_, _| {
            panic!("bounded backoff must block immediate retry")
        })
        .unwrap());
        assert!(
            deliver_with(&store, "event-1", 1_799_999_720_000, |_, settings| {
                assert!(!settings.notifications_show_details);
                Ok(())
            })
            .unwrap()
        );
        store
            .lock()
            .unwrap()
            .transact(|data| {
                data.settings.notifications_enabled = false;
                Ok(())
            })
            .unwrap();
        assert!(!deliver_with(&store, "event-1", 1_799_999_700_200, |_, _| {
            panic!("disabled consent must not call sender")
        })
        .unwrap());
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn sender_rejections_use_persistent_three_attempt_backoff_and_suppress_clock_replay() {
        let (store, path) = fixture_store("bounded-backoff", 1_800_000_000_000);
        let base = 1_799_999_700_000_u128;
        let attempts_at = [base, base + 20_000, base + 80_000];
        for (index, timestamp) in attempts_at.iter().enumerate() {
            assert!(deliver_with(&store, "event-1", *timestamp, |_, _| {
                Err("Private appointment should not appear in error logs".into())
            })
            .is_err());
            let state = store.lock().unwrap();
            let retry = state.data.reminder_retries.values().next().unwrap();
            assert_eq!(retry.attempts, (index + 1) as u8);
            assert_eq!(retry.retry_after_ms, timestamp + RETRY_DELAYS_MS[index]);
            assert!(state.data.reminder_deliveries.is_empty());
            drop(state);
            assert!(!deliver_with(&store, "event-1", *timestamp + 1, |_, _| {
                panic!("Retry blocked before deadline")
            })
            .unwrap());
        }
        assert!(!deliver_with(&store, "event-1", base + 280_000, |_, _| {
            panic!("Three failed attempts should suppress further sends")
        })
        .unwrap());
        assert_eq!(
            due_event_ids(&store.lock().unwrap().data, base + 280_000).len(),
            0
        );
        let restored = Mutex::new(Store::open(path.clone()).unwrap());
        assert!(!deliver_with(&restored, "event-1", base + 280_000, |_, _| {
            panic!("Recovered retry journal must suppress replay")
        })
        .unwrap());
        let current = restored.lock().unwrap().data.clone();
        let mut backed_up = AppData::default();
        crate::preserve_reminder_history(&mut backed_up, &current);
        assert_eq!(
            backed_up.reminder_retries.values().next().unwrap().attempts,
            3
        );
        backed_up.events = current.events;
        backed_up.settings = current.settings;
        assert!(due_event_ids(&backed_up, base + 280_000).is_empty());
        drop(restored);
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn successful_retry_clears_error_history_but_retains_durable_reservation() {
        let (store, path) = fixture_store("retry-success", 1_800_000_000_000);
        let base = 1_799_999_700_000_u128;
        assert!(deliver_with(&store, "event-1", base, |_, _| Err("rejected".into())).is_err());
        assert_eq!(store.lock().unwrap().data.reminder_retries.len(), 1);
        assert!(
            deliver_with(&store, "event-1", base + 20_000, |event, settings| {
                assert_eq!(
                    event_message(event, settings.notifications_show_details),
                    "You have an upcoming calendar event. Open WFS Notch to view it."
                );
                Ok(())
            })
            .unwrap()
        );
        assert!(store.lock().unwrap().data.reminder_retries.is_empty());
        assert_eq!(store.lock().unwrap().data.reminder_deliveries.len(), 1);
        let restored = Mutex::new(Store::open(path.clone()).unwrap());
        assert!(!deliver_with(&restored, "event-1", base + 60_000, |_, _| {
            panic!("Confirmed enqueue must stay suppressed after restart")
        })
        .unwrap());
        drop(restored);
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
    }

    #[test]
    fn retry_parser_validation_and_defensive_backup_merge_preserve_privacy() {
        let valid = ReminderRetry {
            attempts: 2,
            retry_after_ms: 1_800_000_000_000,
            expires_at_ms: 1_802_000_000_000,
        };
        assert!(valid.valid());
        assert!(!ReminderRetry {
            attempts: 0,
            ..valid.clone()
        }
        .valid());
        assert!(!ReminderRetry {
            attempts: 4,
            ..valid.clone()
        }
        .valid());
        assert!(!ReminderRetry {
            retry_after_ms: valid.expires_at_ms + 1,
            ..valid.clone()
        }
        .valid());
        let mut current = AppData::default();
        current
            .reminder_retries
            .insert("opaque-event-key".into(), valid.clone());
        let mut recovered = AppData::default();
        recovered.reminder_retries.insert(
            "opaque-event-key".into(),
            ReminderRetry {
                attempts: 1,
                retry_after_ms: valid.retry_after_ms - 60_000,
                expires_at_ms: valid.expires_at_ms - 60_000,
            },
        );
        crate::preserve_reminder_history(&mut recovered, &current);
        assert_eq!(
            recovered.reminder_retries.get("opaque-event-key"),
            Some(&valid)
        );
        let json = serde_json::to_string(&recovered).unwrap();
        assert!(!json.contains("Private appointment"));
    }

    #[test]
    fn permission_status_mapping_does_not_assume_granted() {
        assert_eq!(describe_native_setting(0), "enabled");
        assert_eq!(describe_native_setting(1), "disabled_for_application");
        assert_eq!(describe_native_setting(3), "disabled_by_policy");
        assert_eq!(describe_native_setting(99), "unknown");
    }

    #[test]
    fn reminder_start_clamps_time_safely() {
        assert_eq!(reminder_start(&event(10_000, Some(60))), Some(0));
        assert_eq!(reminder_start(&event(10_000, None)), None);
    }
}
