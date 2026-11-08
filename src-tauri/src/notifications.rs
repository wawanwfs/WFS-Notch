//! Opt-in WFS-owned Windows notifications, including a native background scheduler.
//! Never accesses notifications from other applications, OS DND, or private calendars
//! outside the local WFS Notch store. All user-facing content is redacted by default.
use crate::{now_ms, with_store, AppData, CalendarEvent, Settings, Store};
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

const POLL_INTERVAL: Duration = Duration::from_secs(20);
const DUE_WINDOW_MS: u128 = 600_000;

#[derive(Default)]
pub struct SentNotices {
    // Expiration by reminder key (event identity + scheduled time + offset).
    // The in-memory ledger avoids repeat toasts while WFS Notch is running.
    ids: Mutex<HashMap<String, u128>>,
}

#[derive(Serialize)]
pub struct ReminderSchedulerStatus {
    enabled: bool,
    paused: bool,
    scheduled_count: usize,
    next_due_at_ms: Option<u64>,
    poll_interval_seconds: u64,
}

fn can_notify(settings: &Settings) -> bool {
    settings.notifications_enabled && !settings.notifications_paused
}

fn reminder_start(event: &CalendarEvent) -> Option<u128> {
    event.remind_minutes.map(|minutes| {
        u128::from(event.starts_at_ms)
            .saturating_sub(u128::from(minutes).saturating_mul(60_000))
    })
}

fn event_due(event: &CalendarEvent, now: u128) -> bool {
    reminder_start(event).is_some_and(|due| now >= due && now < due.saturating_add(DUE_WINDOW_MS))
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
        && !already_sent.contains_key(&delivery_key(event))
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

fn show(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| format!("Windows notification unavailable: {error}"))
}

/// All native reminder entry points share the same deduplication and consent checks.
/// Locks are acquired in the same store -> sent order for the whole operation, so
/// changes to opt-in and pause cannot race the final notification dispatch.
fn deliver_by_id(
    app: &AppHandle,
    store: &Mutex<Store>,
    sent: &SentNotices,
    id: &str,
) -> Result<bool, String> {
    if id.is_empty() || id.len() > 160 {
        return Err("Invalid calendar reminder id".into());
    }
    let guard = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?;
    if !can_notify(&guard.data.settings) {
        return Ok(false);
    }
    let Some(event) = guard.data.events.iter().find(|event| event.id == id) else {
        return Ok(false);
    };
    let now = now_ms();
    let mut ledger = sent
        .ids
        .lock()
        .map_err(|_| "Notification lock unavailable".to_owned())?;
    ledger.retain(|_, expires_at| *expires_at > now);
    if !needs_delivery(event, &guard.data.settings, now, &ledger) {
        return Ok(false);
    }
    // Mark only after successful OS delivery, so a transient Windows error can retry.
    show(
        app,
        "WFS Notch · Calendar",
        &event_message(event, guard.data.settings.notifications_show_details),
    )?;
    let expires_at = reminder_start(event)
        .unwrap_or(now)
        .saturating_add(DUE_WINDOW_MS);
    ledger.insert(delivery_key(event), expires_at);
    Ok(true)
}

/// Single scheduler pass; no window references or JS callbacks are involved.
/// This keeps reminders active when the notch is collapsed or its WebView is hidden.
fn poll_due_events(app: &AppHandle) -> Result<usize, String> {
    let store = app.state::<Mutex<Store>>();
    let due_ids = {
        let guard = store
            .lock()
            .map_err(|_| "Storage lock unavailable".to_owned())?;
        if !can_notify(&guard.data.settings) {
            return Ok(0);
        }
        let now = now_ms();
        guard
            .data
            .events
            .iter()
            .filter(|event| event_due(event, now))
            .map(|event| event.id.clone())
            .take(256)
            .collect::<Vec<_>>()
    };
    let sent = app.state::<SentNotices>();
    let mut delivered = 0;
    for id in due_ids {
        match deliver_by_id(app, store.inner(), sent.inner(), &id) {
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
        .filter_map(|event| reminder_start(event))
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
    sent: State<'_, SentNotices>,
    id: String,
) -> Result<bool, String> {
    deliver_by_id(&app, store.inner(), sent.inner(), &id)
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
        assert!(!event_due(&upcoming, 1_200_000));
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
    }

    #[test]
    fn sensitive_title_is_hidden_by_default() {
        let e = event(1_000_000, Some(15));
        let hidden = event_message(&e, false);
        assert!(!hidden.contains("Private appointment"));
        assert!(event_message(&e, true).contains("Private appointment"));
    }

    #[test]
    fn reminder_start_clamps_time_safely() {
        assert_eq!(reminder_start(&event(10_000, Some(60))), Some(0));
        assert_eq!(reminder_start(&event(10_000, None)), None);
    }
}
