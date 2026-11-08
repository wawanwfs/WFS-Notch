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
    artwork_data_url: Option<String>,
}

fn non_negative_ms(time_100ns: i64) -> u64 {
    (time_100ns.max(0) / 10_000) as u64
}

fn normalized_timeline(start: i64, position: i64, end: i64) -> (u64, u64) {
    // GSMTC timelines may start at a nonzero 100ns offset.
    let total = non_negative_ms(end.saturating_sub(start));
    let elapsed = non_negative_ms(position.saturating_sub(start)).min(total);
    (elapsed, total)
}

fn artwork_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}

#[cfg(windows)]
fn artwork_data_url(
    properties: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<String> {
    use base64::Engine;
    use windows::Storage::Streams::DataReader;

    const MAX_ARTWORK_BYTES: u64 = 512 * 1024;
    let reference = properties.Thumbnail().ok()?;
    let stream = reference.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_ARTWORK_BYTES {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    let loaded = reader.LoadAsync(size as u32).ok()?.join().ok()?;
    if loaded == 0 || u64::from(loaded) > size {
        return None;
    }
    let mut image = vec![0_u8; loaded as usize];
    reader.ReadBytes(&mut image).ok()?;
    let mime = artwork_mime(&image)?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(image)
    ))
}

#[cfg(windows)]
fn read_session_with_source(
    requested_source: Option<&str>,
) -> Result<Option<ActiveMediaSnapshot>, String> {
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
    let session = if let Some(requested) = requested_source {
        let sessions = manager.GetSessions().map_err(|e| e.to_string())?;
        let mut selected = None;
        for i in 0..sessions.Size().map_err(|e| e.to_string())?.min(64) {
            let Ok(candidate) = sessions.GetAt(i) else {
                continue;
            };
            if candidate
                .SourceAppUserModelId()
                .ok()
                .map(|s| s.to_string())
                .as_deref()
                == Some(requested)
            {
                selected = Some(candidate);
                break;
            }
        }
        match selected {
            Some(session) => session,
            None => return Ok(None), // Disconnected; never substitute another player.
        }
    } else {
        match manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => return Ok(None),
        }
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
    let (position_ms, duration_ms) = timeline
        .as_ref()
        .and_then(|time| {
            let start = time.StartTime().ok()?.Duration;
            let position = time.Position().ok()?.Duration;
            let end = time.EndTime().ok()?.Duration;
            Some(normalized_timeline(start, position, end))
        })
        .map_or((None, None), |(pos, total)| (Some(pos), Some(total)));
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
        artwork_data_url: artwork_data_url(&properties),
    }))
}

#[cfg(windows)]
fn read_session() -> Result<Option<ActiveMediaSnapshot>, String> {
    read_session_with_source(None)
}

#[cfg(not(windows))]
fn read_session() -> Result<Option<ActiveMediaSnapshot>, String> {
    Err("Native media sessions are available only on Windows".into())
}

#[cfg(not(windows))]
fn read_session_with_source(_source: Option<&str>) -> Result<Option<ActiveMediaSnapshot>, String> {
    Err("Native media sessions are available only on Windows".into())
}

#[tauri::command]
pub async fn get_media_session_by_source(
    source: String,
) -> Result<Option<ActiveMediaSnapshot>, String> {
    if source.is_empty() || source.len() > 256 {
        return Err("Invalid media source".into());
    }
    tauri::async_runtime::spawn_blocking(move || read_session_with_source(Some(&source)))
        .await
        .map_err(|error| format!("Media session worker failed: {error}"))?
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
    fn artwork_mime_strictly_limits_local_image_types() {
        assert_eq!(
            artwork_mime(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            Some("image/png")
        );
        assert_eq!(artwork_mime(&[0xff, 0xd8, 0xff, 0x00]), Some("image/jpeg"));
        assert_eq!(artwork_mime(b"GIF89a"), Some("image/gif"));
        assert_eq!(artwork_mime(b"<svg onload=alert(1)>"), None);
        assert_eq!(artwork_mime(b"<html>"), None);
    }

    #[test]
    fn progress_is_clamped_to_track_and_handles_nonzero_start() {
        assert_eq!(normalized_timeline(100_000, 600_000, 1_100_000), (50, 100));
        assert_eq!(normalized_timeline(100_000, 0, 1_100_000), (0, 100));
        assert_eq!(
            normalized_timeline(100_000, 2_000_000, 1_100_000),
            (100, 100)
        );
        assert_eq!(normalized_timeline(100_000, 0, 50_000), (0, 0));
    }

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
    can_toggle: bool,
    can_next: bool,
    can_previous: bool,
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
        if source.is_empty()
            || source.len() > 256
            || result
                .iter()
                .any(|item: &MediaSessionChoice| item.source == source)
        {
            continue;
        }
        let properties = session
            .TryGetMediaPropertiesAsync()
            .ok()
            .and_then(|operation| operation.join().ok());
        let title = properties
            .as_ref()
            .and_then(|p| p.Title().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let artist = properties
            .as_ref()
            .and_then(|p| p.Artist().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let status = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|information| information.PlaybackStatus().ok())
            .map(|status| {
                if status == Status::Playing {
                    "Playing"
                } else if status == Status::Paused {
                    "Paused"
                } else {
                    "Inactive"
                }
            })
            .unwrap_or("Unknown")
            .to_owned();
        let controls = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|info| info.Controls().ok());
        result.push(MediaSessionChoice {
            source,
            title,
            artist,
            playback_status: status,
            can_toggle: controls
                .as_ref()
                .and_then(|c| c.IsPlayPauseToggleEnabled().ok())
                .unwrap_or(false),
            can_next: controls
                .as_ref()
                .and_then(|c| c.IsNextEnabled().ok())
                .unwrap_or(false),
            can_previous: controls
                .as_ref()
                .and_then(|c| c.IsPreviousEnabled().ok())
                .unwrap_or(false),
        });
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
        let id = session
            .SourceAppUserModelId()
            .map(|v| v.to_string())
            .unwrap_or_default();
        if id != source {
            continue;
        }
        // Query controls again on the *current* session, not stale UI metadata.
        let controls = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|info| info.Controls().ok());
        let supported = match action {
            "toggle" => controls
                .as_ref()
                .and_then(|c| c.IsPlayPauseToggleEnabled().ok()),
            "next" => controls.as_ref().and_then(|c| c.IsNextEnabled().ok()),
            "previous" => controls.as_ref().and_then(|c| c.IsPreviousEnabled().ok()),
            _ => None,
        };
        if supported != Some(true) {
            return Ok(false);
        }
        let operation = match action {
            "toggle" => session.TryTogglePlayPauseAsync(),
            "next" => session.TrySkipNextAsync(),
            "previous" => session.TrySkipPreviousAsync(),
            _ => return Err("Unsupported media action".into()),
        }
        .map_err(|e| format!("Media player rejected control: {e}"))?;
        return operation
            .join()
            .map_err(|e| format!("Media player did not respond: {e}"));
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
