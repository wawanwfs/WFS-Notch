//! Opt-in WFS-owned Windows notifications, including a native background scheduler.
//! Never accesses notifications from other applications, OS DND, or private calendars
//! outside the local WFS Notch store. All user-facing content is redacted by default.
use crate::{now_ms, with_store, AppData, CalendarEvent, Settings, Store};
use serde::Serialize;
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

#[derive(Serialize)]
pub struct ReminderSchedulerStatus {
    enabled: bool,
    paused: bool,
    scheduled_count: usize,
    next_due_at_ms: Option<u64>,
    poll_interval_seconds: u64,
    persisted_delivery_count: usize,
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
        .filter(|event| needs_delivery(event, &data.settings, now, &data.reminder_deliveries))
        .map(|event| event.id.clone())
        .take(256)
        .collect()
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
fn deliver_with<F>(
    store: &Mutex<Store>,
    id: &str,
    now: u128,
    sender: F,
) -> Result<bool, String>
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
    if !needs_delivery(&event, &settings, now, &guard.data.reminder_deliveries) {
        return Ok(false);
    }
    let key = delivery_key(&event);
    let expires_at = reservation_expires_at(&event);
    // Store::transact writes to an fsync'd temporary state and renames it.
    guard.transact(|data| {
        data.reminder_deliveries.retain(|_, until| *until > now);
        data.reminder_deliveries.insert(key.clone(), expires_at);
        Ok(())
    })?;
    if let Err(error) = sender(&event, &settings) {
        // OS API rejected the enqueue. Revert reservation so the native worker
        // can retry on its next 20-second pass, assuming the event remains due.
        guard.transact(|data| {
            data.reminder_deliveries.remove(&key);
            Ok(())
        })?;
        return Err(error);
    }
    Ok(true)
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
    let due_ids = {
        let guard = store
            .lock()
            .map_err(|_| "Storage lock unavailable".to_owned())?;
        due_event_ids(&guard.data, now_ms())
    };
    let mut delivered = 0;
    for id in due_ids {
        match deliver_by_id(app, store.inner(), &id) {
            Ok(true) => delivered += 1,
            Ok(false) => {}
            Err(error) => {
                // No calendar titles or private event data are written to logs.
                eprintln!("WFS reminder delivery failed: {error}");
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
        poll_interval_seconds: POLL_INTERVAL.as_secs(),
        persisted_delivery_count: guard
            .data
            .reminder_deliveries
            .values()
            .filter(|expiry| **expiry > now)
            .count(),
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

    fn event(starts_at_ms: u64, remind_minutes: Option<u32>) -> CalendarEvent {
        CalendarEvent {
            id: "event-1".into(),
            title: "Private appointment".into(),
            starts_at_ms,
            remind_minutes,
            description: String::new(),
            created_ms: 0,
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
        let path = std::env::temp_dir().join(format!(
            "wfs-phase13-{}-{}.json",
            crate::new_id(),
            id
        ));
        let mut store = Store::open(path.clone()).unwrap();
        store.transact(|data| {
            data.settings.notifications_enabled = true;
            data.events.push(event(time, Some(5)));
            Ok(())
        }).unwrap();
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
        data.reminder_deliveries.insert(delivery_key(&first), reservation_expires_at(&first));
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
        }).unwrap());
        assert_eq!(sends, 1);
        // Repeated polls and application restarts must not duplicate attempts.
        assert!(!deliver_with(&store, "event-1", 1_799_999_710_000, |_, _| {
            sends += 1;
            Ok(())
        }).unwrap());
        let reloaded = Mutex::new(Store::open(path.clone()).unwrap());
        assert!(!deliver_with(&reloaded, "event-1", 1_799_999_700_000, |_, _| {
            sends += 1;
            Ok(())
        }).unwrap());
        assert_eq!(sends, 1);
        reloaded.lock().unwrap().transact(|data| {
            data.events[0].starts_at_ms = 1_800_000_100_000;
            Ok(())
        }).unwrap();
        assert!(deliver_with(&reloaded, "event-1", 1_799_999_800_000, |_, _| {
            sends += 1;
            Ok(())
        }).unwrap());
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
        }).is_err());
        assert!(store.lock().unwrap().data.reminder_deliveries.is_empty());
        assert!(deliver_with(&store, "event-1", 1_799_999_700_100, |_, settings| {
            assert!(!settings.notifications_show_details);
            Ok(())
        }).unwrap());
        store.lock().unwrap().transact(|data| {
            data.settings.notifications_enabled = false;
            Ok(())
        }).unwrap();
        assert!(!deliver_with(&store, "event-1", 1_799_999_700_200, |_, _| {
            panic!("disabled consent must not call sender")
        }).unwrap());
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
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
