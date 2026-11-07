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
    let properties = session.TryGetMediaPropertiesAsync()
        .map_err(|error| format!("Media properties unavailable: {error}"))?
        .join()
        .map_err(|error| format!("Cannot read current track: {error}"))?;

    let status = session.GetPlaybackInfo()
        .ok()
        .and_then(|info| info.PlaybackStatus().ok())
        .map(|value| {
            if value == Status::Playing { "Playing" }
            else if value == Status::Paused { "Paused" }
            else if value == Status::Stopped { "Stopped" }
            else { "Inactive" }
        })
        .unwrap_or("Unknown")
        .to_owned();

    let timeline = session.GetTimelineProperties().ok();
    let position_ms = timeline.as_ref()
        .and_then(|timeline| timeline.Position().ok())
        .map(|value| non_negative_ms(value.Duration));
    let duration_ms = timeline.as_ref()
        .and_then(|timeline| {
            let end = timeline.EndTime().ok()?.Duration;
            let start = timeline.StartTime().ok()?.Duration;
            Some(non_negative_ms(end.saturating_sub(start)))
        });
    Ok(Some(ActiveMediaSnapshot {
        source: session.SourceAppUserModelId().map(|x| x.to_string()).unwrap_or_default(),
        title: properties.Title().map(|x| x.to_string()).unwrap_or_default(),
        artist: properties.Artist().map(|x| x.to_string()).unwrap_or_default(),
        album: properties.AlbumTitle().map(|x| x.to_string()).unwrap_or_default(),
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
