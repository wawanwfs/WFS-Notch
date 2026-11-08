//! Persistent, DPI-aware notch placement on connected Windows displays.
//! No display configuration changes or privileged Windows API calls are made.
use crate::{with_store, AppData, Settings, Store};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, State, WebviewWindow};
use std::time::Duration;

const COLLAPSED_WIDTH: f64 = 240.0;
const COLLAPSED_HEIGHT: f64 = 48.0;
const EXPANDED_WIDTH: f64 = 760.0;
const EXPANDED_HEIGHT: f64 = 470.0;
pub const DEFAULT_MONITOR: &str = "primary";
pub const MAX_MARGIN: u32 = 120;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayOption {
    id: String,
    name: String,
    width: u32,
    height: u32,
    scale_factor: f64,
    is_primary: bool,
}

#[derive(Debug, Serialize)]
pub struct DisplaySnapshot {
    displays: Vec<DisplayOption>,
    monitor_id: String,
    margin_px: u32,
    active_monitor_id: String,
    using_fallback: bool,
}

#[derive(Debug, PartialEq)]
struct Placement {
    logical_width: f64,
    logical_height: f64,
    x: i32,
    y: i32,
}

fn monitor_id(monitor: &tauri::Monitor) -> String {
    let position = monitor.position();
    format!("origin:{}:{}", position.x, position.y)
}

fn select_monitor(
    window: &WebviewWindow,
    preference: &str,
) -> Result<(Option<tauri::Monitor>, bool), String> {
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    let preferred = if preference == DEFAULT_MONITOR {
        window.primary_monitor().map_err(|e| e.to_string())?
    } else {
        monitors
            .iter()
            .find(|monitor| monitor_id(monitor) == preference)
            .cloned()
    };
    if preferred.is_some() {
        return Ok((preferred, false));
    }
    let fallback = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .or(window.current_monitor().map_err(|e| e.to_string())?)
        .or_else(|| monitors.into_iter().next());
    Ok((fallback, true))
}

fn geometry(
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f64,
    margin: u32,
    expanded: bool,
) -> Placement {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let desired_width = if expanded {
        EXPANDED_WIDTH
    } else {
        COLLAPSED_WIDTH
    };
    let desired_height = if expanded {
        EXPANDED_HEIGHT
    } else {
        COLLAPSED_HEIGHT
    };
    let available_width = (monitor_width as f64 / scale - 16.0).max(200.0);
    let available_height = (monitor_height as f64 / scale - 24.0).max(40.0);
    let logical_width = desired_width.min(available_width);
    let logical_height = desired_height.min(available_height);
    let pixel_width = (logical_width * scale).round() as i64;
    let pixel_height = (logical_height * scale).round() as i64;
    let x = ((i64::from(monitor_width) - pixel_width).max(0) / 2) as i32;
    let y = ((f64::from(margin.min(MAX_MARGIN)) * scale).round() as i64)
        .min((i64::from(monitor_height) - pixel_height).max(0)) as i32;
    Placement {
        logical_width,
        logical_height,
        x,
        y,
    }
}

pub fn place_notch(
    window: &WebviewWindow,
    settings: &Settings,
    expanded: bool,
) -> Result<(), String> {
    let (monitor, _) = select_monitor(window, &settings.monitor_id)?;
    let Some(monitor) = monitor else {
        // No connected display could be reported (e.g. remote/virtual headless session).
        // Preserve the last working position instead of guessing a screen origin.
        let size = if expanded {
            (EXPANDED_WIDTH, EXPANDED_HEIGHT)
        } else {
            (COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
        };
        return window
            .set_size(tauri::LogicalSize::new(size.0, size.1))
            .map_err(|e| e.to_string());
    };
    let target = geometry(
        monitor.size().width,
        monitor.size().height,
        monitor.scale_factor(),
        settings.top_margin_px,
        expanded,
    );
    window
        .set_size(tauri::LogicalSize::new(
            target.logical_width,
            target.logical_height,
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(
            monitor.position().x.saturating_add(target.x),
            monitor.position().y.saturating_add(target.y),
        ))
        .map_err(|e| e.to_string())
}


#[cfg(windows)]
fn active_fullscreen_on_monitor(window: &WebviewWindow, settings: &Settings) -> bool {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO,
        MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };
    let Ok((Some(monitor), _)) = select_monitor(window, &settings.monitor_id) else {
        return false;
    };
    let position = monitor.position();
    let size = monitor.size();
    let point = POINT {
        x: position.x.saturating_add((size.width / 2) as i32),
        y: position.y.saturating_add((size.height / 2) as i32),
    };
    // SAFETY: read-only Win32 foreground/window/monitor queries, owned POD buffers.
    unsafe {
        let foreground = GetForegroundWindow();
        let mut process_id = 0_u32;
        if GetWindowThreadProcessId(foreground, Some(&mut process_id)) == 0
            || process_id == std::process::id()
        {
            return false; // WFS Notch itself is focused.
        }
        let target = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let current = MonitorFromWindow(foreground, MONITOR_DEFAULTTONULL);
        if target != current {
            return false; // Fullscreen app on a *different* monitor.
        }
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut rect = RECT::default();
        if !GetMonitorInfoW(current, &mut monitor_info).as_bool()
            || GetWindowRect(foreground, &mut rect).is_err()
        {
            return false;
        }
        covers_monitor(
            (rect.left, rect.top, rect.right, rect.bottom),
            (
                monitor_info.rcMonitor.left,
                monitor_info.rcMonitor.top,
                monitor_info.rcMonitor.right,
                monitor_info.rcMonitor.bottom,
            ),
        )
    }
}
#[cfg(not(windows))]
fn active_fullscreen_on_monitor(_window: &WebviewWindow, _settings: &Settings) -> bool {
    false
}

// 2px margin tolerates Win32 invisible borders; small maximized apps do not
// qualify because the taskbar leaves an uncovered strip.
fn covers_monitor(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32)) -> bool {
    window.0 <= monitor.0.saturating_add(2)
        && window.1 <= monitor.1.saturating_add(2)
        && window.2 >= monitor.2.saturating_sub(2)
        && window.3 >= monitor.3.saturating_sub(2)
}

/// Reorder without activating/focusing the app; never call set_focus here.
pub fn sync_topmost(window: &WebviewWindow, settings: &Settings) -> Result<(), String> {
    let allow = settings.always_on_top
        && !(settings.auto_lower_fullscreen && active_fullscreen_on_monitor(window, settings));
    window.set_always_on_top(allow).map_err(|e| e.to_string())
}

/// Poll only local window and display geometry. No app-title/process-name
/// collection, no accessibility hooks, no global keyboard or screen capture.
pub fn start_guard(app: AppHandle) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("wfs-display-guard".into())
        .spawn(move || {
            let mut previous_geometry: Option<Vec<String>> = None;
            let mut previous_topmost: Option<bool> = None;
            loop {
                if let Some(window) = app.get_webview_window("main") {
                    let store = app.state::<Mutex<Store>>();
                    let settings = store.lock().ok().map(|state| state.data.settings.clone());
                    if let Some(settings) = settings {
                        if let Ok(monitors) = window.available_monitors() {
                            let mut fingerprint = monitors
                                .iter()
                                .map(|m| format!(
                                    "{}:{}:{}:{}:{:.3}",
                                    m.position().x, m.position().y,
                                    m.size().width, m.size().height, m.scale_factor()
                                ))
                                .collect::<Vec<_>>();
                            fingerprint.sort();
                            if previous_geometry.as_ref().is_some_and(|old| old != &fingerprint) {
                                let height = window.inner_size().ok().map(|s| s.height as f64);
                                let scale = window.scale_factor().unwrap_or(1.0).max(0.1);
                                let expanded = height.is_some_and(|h| h / scale > 120.0);
                                if let Err(error) = place_notch(&window, &settings, expanded) {
                                    eprintln!("WFS display placement unavailable: {error}");
                                }
                            }
                            previous_geometry = Some(fingerprint);
                        }
                        let desired = settings.always_on_top
                            && !(settings.auto_lower_fullscreen
                                && active_fullscreen_on_monitor(&window, &settings));
                        if previous_topmost != Some(desired) {
                            if let Err(error) = window.set_always_on_top(desired) {
                                eprintln!("WFS topmost update unavailable: {error}");
                            } else {
                                previous_topmost = Some(desired);
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        })
        .map(|_| ())
}

#[tauri::command]
pub fn get_display_options(
    window: WebviewWindow,
    store: State<'_, Mutex<Store>>,
) -> Result<DisplaySnapshot, String> {
    let settings = store
        .lock()
        .map_err(|_| "Data lock is unavailable".to_owned())?
        .data
        .settings
        .clone();
    let primary_id = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(monitor_id);
    let (active, using_fallback) = select_monitor(&window, &settings.monitor_id)?;
    let mut displays = window
        .available_monitors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|monitor| DisplayOption {
            id: monitor_id(&monitor),
            name: monitor
                .name()
                .cloned()
                .unwrap_or_else(|| "Unnamed display".into()),
            width: monitor.size().width,
            height: monitor.size().height,
            scale_factor: monitor.scale_factor(),
            is_primary: primary_id.as_deref() == Some(monitor_id(&monitor).as_str()),
        })
        .collect::<Vec<_>>();
    displays.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(DisplaySnapshot {
        displays,
        monitor_id: settings.monitor_id,
        margin_px: settings.top_margin_px,
        active_monitor_id: active.as_ref().map(monitor_id).unwrap_or_default(),
        using_fallback,
    })
}

#[tauri::command]
pub fn set_display_preference(
    window: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    monitor_id: String,
    margin_px: u32,
) -> Result<AppData, String> {
    if margin_px > MAX_MARGIN {
        return Err("Top margin must be between 0 and 120 logical pixels".into());
    }
    if monitor_id != DEFAULT_MONITOR {
        let connected = window
            .available_monitors()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|monitor| self::monitor_id(monitor) == monitor_id);
        if !connected {
            return Err("Selected display is disconnected. Select Primary display instead.".into());
        }
    }
    // Use the active window's logical dimensions to preserve collapsed/expanded state.
    let current_height = window.inner_size().map_err(|e| e.to_string())?.height as f64;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let expanded = current_height / scale > 120.0;
    with_store(store, |data| {
        data.settings.monitor_id = monitor_id;
        data.settings.top_margin_px = margin_px;
        place_notch(&window, &data.settings, expanded)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_centers_and_preserves_negative_virtual_origins() {
        let value = geometry(1920, 1080, 1.0, 12, true);
        assert_eq!(value.logical_width, 760.0);
        assert_eq!(value.x, 580);
        assert_eq!(value.y, 12);
        // A separate monitor may be at x=-1920; caller adds that signed origin.
        assert_eq!((-1920_i32).saturating_add(value.x), -1340);
    }

    #[test]
    fn high_dpi_and_narrow_display_are_clamped() {
        let high_dpi = geometry(1920, 1080, 1.5, 12, false);
        assert_eq!(high_dpi.x, 780);
        assert_eq!(high_dpi.y, 18);
        let narrow = geometry(640, 480, 1.0, 120, true);
        assert!(narrow.logical_width <= 624.0);
        assert!(narrow.logical_height <= 456.0);
        assert!(narrow.y >= 0);
        assert!(narrow.y + narrow.logical_height as i32 <= 480);
    }

    #[test]
    fn fullscreen_tolerance_does_not_match_maximized_work_area() {
        assert!(covers_monitor((0, 0, 1920, 1080), (0, 0, 1920, 1080)));
        assert!(covers_monitor((-1922, -1, -1, 1081), (-1920, 0, 0, 1080)));
        assert!(!covers_monitor((0, 0, 1920, 1040), (0, 0, 1920, 1080)));
        assert!(!covers_monitor((1920, 0, 3840, 1080), (0, 0, 1920, 1080)));
    }

    #[test]
    fn invalid_scale_uses_safe_default() {
        let value = geometry(1920, 1080, 0.0, u32::MAX, false);
        assert_eq!(value.logical_width, COLLAPSED_WIDTH);
        assert_eq!(value.y, MAX_MARGIN as i32);
    }
}
