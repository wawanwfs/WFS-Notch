//! Opt-in WFS-owned Windows notifications (never reads other apps' messages).
//! Notification detail text is redacted by default. Native delivery requires an installed app.
use crate::{now_ms, with_store, AppData, CalendarEvent, Settings, Store};
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;

#[derive(Default)]
pub struct SentNotices {
    ids: Mutex<HashSet<String>>,
}

fn event_due(event: &CalendarEvent, now: u128) -> bool {
    let Some(minutes) = event.remind_minutes else {
        return false;
    };
    let due =
        u128::from(event.starts_at_ms).saturating_sub(u128::from(minutes).saturating_mul(60_000));
    now >= due && now < due.saturating_add(600_000)
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

#[tauri::command]
pub fn set_notification_preferences(
    store: State<'_, Mutex<Store>>,
    enabled: bool,
    show_details: bool,
) -> Result<AppData, String> {
    with_store(store, |data| {
        data.settings.notifications_enabled = enabled;
        // Turning off notifications revokes explicit consent to show details as well.
        data.settings.notifications_show_details = enabled && show_details;
        Ok(())
    })
}

#[tauri::command]
pub fn notify_test(app: AppHandle, store: State<'_, Mutex<Store>>) -> Result<bool, String> {
    let enabled = store
        .lock()
        .map_err(|_| "Storage lock unavailable".to_owned())?
        .data
        .settings
        .notifications_enabled;
    if !enabled {
        return Ok(false);
    }
    show(
        &app,
        "WFS Notch",
        "Desktop notifications are enabled. Your private information remains hidden by default.",
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
    if id.len() > 160 || id.is_empty() {
        return Err("Invalid calendar reminder id".into());
    }
    let (event, settings): (CalendarEvent, Settings) = {
        let guard = store
            .lock()
            .map_err(|_| "Storage lock unavailable".to_owned())?;
        if !guard.data.settings.notifications_enabled {
            return Ok(false);
        }
        let Some(event) = guard.data.events.iter().find(|e| e.id == id) else {
            return Ok(false);
        };
        (event.clone(), guard.data.settings.clone())
    };
    if !event_due(&event, now_ms()) {
        return Ok(false);
    }
    let mut seen = sent
        .ids
        .lock()
        .map_err(|_| "Notification lock unavailable".to_owned())?;
    if seen.contains(&id) {
        return Ok(false);
    }
    // Mark only on successful native delivery, preventing accidental duplicate toasts.
    show(
        &app,
        "WFS Notch · Calendar",
        &event_message(&event, settings.notifications_show_details),
    )?;
    if seen.len() >= 2048 {
        seen.clear();
    }
    seen.insert(id);
    Ok(true)
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
    fn sensitive_title_is_hidden_by_default() {
        let e = event(1_000_000, Some(15));
        let hidden = event_message(&e, false);
        assert!(!hidden.contains("Private appointment"));
        assert!(event_message(&e, true).contains("Private appointment"));
    }
}
