//! Opt-in, read-only Windows Global System Media Transport Controls session snapshot.
//! No microphone access, audio capture, files, credentials, or network calls.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ActiveMediaSnapshot {
    source: String,
    title: String,
    artist: String,
    album: String,
    playback_status: String,
    position_ms: Option<u64>,
    duration_ms: Option<u64>,
}

fn non_negative_ms(time_100ns: i64) -> u64 {
    (time_100ns.max(0) / 10_000) as u64
}

#[cfg(windows)]
fn read_session() -> Result<Option<ActiveMediaSnapshot>, String> {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as MediaManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    // A blocking thread is used, never the Tauri UI thread. Repeated polling only happens
    // while the user explicitly opens the Media panel, not in the background.
    let manager = MediaManager::RequestAsync()
        .map_err(|error| format!("Windows media manager unavailable: {error}"))?
        .join()
        .map_err(|error| format!("Cannot access Windows media sessions: {error}"))?;
    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        Err(_) => return Ok(None),
    };
    let properties = session
        .TryGetMediaPropertiesAsync()
        .map_err(|error| format!("Media properties unavailable: {error}"))?
        .join()
        .map_err(|error| format!("Cannot read current track: {error}"))?;

    let status = session
        .GetPlaybackInfo()
        .ok()
        .and_then(|info| info.PlaybackStatus().ok())
        .map(|value| {
            if value == Status::Playing {
                "Playing"
            } else if value == Status::Paused {
                "Paused"
            } else if value == Status::Stopped {
                "Stopped"
            } else {
                "Inactive"
            }
        })
        .unwrap_or("Unknown")
        .to_owned();

    let timeline = session.GetTimelineProperties().ok();
    let position_ms = timeline
        .as_ref()
        .and_then(|timeline| timeline.Position().ok())
        .map(|value| non_negative_ms(value.Duration));
    let duration_ms = timeline.as_ref().and_then(|timeline| {
        let end = timeline.EndTime().ok()?.Duration;
        let start = timeline.StartTime().ok()?.Duration;
        Some(non_negative_ms(end.saturating_sub(start)))
    });
    Ok(Some(ActiveMediaSnapshot {
        source: session
            .SourceAppUserModelId()
            .map(|x| x.to_string())
            .unwrap_or_default(),
        title: properties
            .Title()
            .map(|x| x.to_string())
            .unwrap_or_default(),
        artist: properties
            .Artist()
            .map(|x| x.to_string())
            .unwrap_or_default(),
        album: properties
            .AlbumTitle()
            .map(|x| x.to_string())
            .unwrap_or_default(),
        playback_status: status,
        position_ms,
        duration_ms,
    }))
}

#[cfg(not(windows))]
fn read_session() -> Result<Option<ActiveMediaSnapshot>, String> {
    Err("Native media sessions are available only on Windows".into())
}

#[tauri::command]
pub async fn get_active_media_session() -> Result<Option<ActiveMediaSnapshot>, String> {
    tauri::async_runtime::spawn_blocking(read_session)
        .await
        .map_err(|error| format!("Media session worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time_conversion_clamps_negative_and_100ns_units() {
        assert_eq!(non_negative_ms(1_234_560), 123);
        assert_eq!(non_negative_ms(-10), 0);
        assert_eq!(non_negative_ms(10_000), 1);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaSessionChoice {
    source: String,
    title: String,
    artist: String,
    playback_status: String,
}

fn valid_control(action: &str) -> bool {
    matches!(action, "toggle" | "next" | "previous")
}

#[cfg(windows)]
fn read_media_choices() -> Result<Vec<MediaSessionChoice>, String> {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as MediaManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    let manager = MediaManager::RequestAsync()
        .map_err(|e| format!("Media manager unavailable: {e}"))?
        .join()
        .map_err(|e| format!("Cannot read Windows sessions: {e}"))?;
    let view = manager.GetSessions().map_err(|e| e.to_string())?;
    let count = view.Size().map_err(|e| e.to_string())?.min(24);
    let mut result = Vec::new();
    for index in 0..count {
        let Ok(session) = view.GetAt(index) else {
            continue;
        };
        let source = session
            .SourceAppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if source.is_empty() || source.len() > 256 || result.iter().any(|item: &MediaSessionChoice| item.source == source) {
            continue;
        }
        let properties = session
            .TryGetMediaPropertiesAsync()
            .ok()
            .and_then(|operation| operation.join().ok());
        let title = properties.as_ref().and_then(|p| p.Title().ok()).map(|s| s.to_string()).unwrap_or_default();
        let artist = properties.as_ref().and_then(|p| p.Artist().ok()).map(|s| s.to_string()).unwrap_or_default();
        let status = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|information| information.PlaybackStatus().ok())
            .map(|status| if status == Status::Playing { "Playing" } else if status == Status::Paused { "Paused" } else { "Inactive" })
            .unwrap_or("Unknown")
            .to_owned();
        result.push(MediaSessionChoice { source, title, artist, playback_status: status });
    }
    Ok(result)
}

#[cfg(not(windows))]
fn read_media_choices() -> Result<Vec<MediaSessionChoice>, String> {
    Err("Windows media session selection is available only on Windows".into())
}

#[tauri::command]
pub async fn list_media_sessions() -> Result<Vec<MediaSessionChoice>, String> {
    tauri::async_runtime::spawn_blocking(read_media_choices)
        .await
        .map_err(|error| format!("Media session worker failed: {error}"))?
}

#[cfg(windows)]
fn apply_media_control(source: &str, action: &str) -> Result<bool, String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as MediaManager;
    let manager = MediaManager::RequestAsync()
        .map_err(|e| format!("Media manager unavailable: {e}"))?
        .join()
        .map_err(|e| format!("Cannot read Windows sessions: {e}"))?;
    let view = manager.GetSessions().map_err(|e| e.to_string())?;
    for index in 0..view.Size().map_err(|e| e.to_string())?.min(64) {
        let session = view.GetAt(index).map_err(|e| e.to_string())?;
        let id = session.SourceAppUserModelId().map(|v| v.to_string()).unwrap_or_default();
        if id != source {
            continue;
        }
        let operation = match action {
            "toggle" => session.TryTogglePlayPauseAsync(),
            "next" => session.TrySkipNextAsync(),
            "previous" => session.TrySkipPreviousAsync(),
            _ => return Err("Unsupported media action".into()),
        }.map_err(|e| format!("Media player rejected control: {e}"))?;
        return operation.join().map_err(|e| format!("Media player did not respond: {e}"));
    }
    Err("The selected media player is no longer available".into())
}

#[cfg(not(windows))]
fn apply_media_control(_source: &str, _action: &str) -> Result<bool, String> {
    Err("Media controls require Windows".into())
}

#[tauri::command]
pub async fn control_media_session(source: String, action: String) -> Result<bool, String> {
    if source.is_empty() || source.len() > 256 || !valid_control(&action) {
        return Err("Invalid media player or playback action".into());
    }
    tauri::async_runtime::spawn_blocking(move || apply_media_control(&source, &action))
        .await
        .map_err(|error| format!("Media control worker failed: {error}"))?
}

#[cfg(test)]
mod phase10_tests {
    use super::*;
    #[test]
    fn media_actions_are_strictly_allowlisted() {
        assert!(valid_control("toggle"));
        assert!(valid_control("next"));
        assert!(valid_control("previous"));
        assert!(!valid_control("record"));
        assert!(!valid_control("delete"));
    }
}
