//! Phase 4 desktop modules. All mutable user content stays in Store's local state.json.
use super::*;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Habit {
    pub id: String,
    pub title: String,
    pub completed_dates: Vec<String>,
    pub created_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickLink {
    pub id: String,
    pub label: String,
    pub url: String,
    pub created_ms: u128,
}

fn validate_title(input: &str) -> Result<String, String> {
    let title = input.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err("Title must contain 1–120 characters".into());
    }
    Ok(title.to_owned())
}

fn validate_day(value: &str) -> Result<(), String> {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 ||
        !value.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
        return Err("Date must use YYYY-MM-DD".into());
    }
    let year: u32 = parts[0].parse().map_err(|_| "Invalid year")?;
    let month: usize = parts[1].parse().map_err(|_| "Invalid month")?;
    let day: u32 = parts[2].parse().map_err(|_| "Invalid day")?;
    if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) {
        return Err("Date is outside supported range".into());
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day == 0 || day > month_days[month - 1] {
        return Err("Invalid calendar date".into());
    }
    Ok(())
}

fn validate_url(input: &str) -> Result<String, String> {
    let raw = input.trim();
    if raw.len() > 2048 || raw.chars().any(char::is_whitespace) {
        return Err("URL must be under 2048 characters without whitespace".into());
    }
    let parsed = url::Url::parse(raw).map_err(|_| "Enter a complete http(s) URL")?;
    if !matches!(parsed.scheme(), "http" | "https") ||
        parsed.host_str().is_none() ||
        !parsed.username().is_empty() ||
        parsed.password().is_some() {
        return Err("Only http(s) links without embedded credentials are allowed".into());
    }
    Ok(parsed.to_string())
}

#[tauri::command]
pub fn add_habit(store: State<'_, Mutex<Store>>, title: String) -> Result<AppData, String> {
    let title = validate_title(&title)?;
    with_store(store, |data| {
        if data.habits.len() >= 200 { return Err("Maximum 200 habits".into()); }
        data.habits.push(Habit {
            id: new_id(), title, completed_dates: Vec::new(), created_ms: now_ms()
        });
        Ok(())
    })
}

#[tauri::command]
pub fn toggle_habit_day(store: State<'_, Mutex<Store>>, id: String, day: String) -> Result<AppData, String> {
    validate_day(&day)?;
    with_store(store, |data| {
        let habit = data.habits.iter_mut().find(|habit| habit.id == id).ok_or("Habit not found")?;
        if let Some(index) = habit.completed_dates.iter().position(|item| item == &day) {
            habit.completed_dates.remove(index);
        } else {
            habit.completed_dates.push(day);
            habit.completed_dates.sort();
            // Limit indefinite growth from excessive or hostile UI calls.
            if habit.completed_dates.len() > 3660 {
                habit.completed_dates.remove(0);
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub fn remove_habit(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |data| {
        let before = data.habits.len();
        data.habits.retain(|item| item.id != id);
        if before == data.habits.len() { return Err("Habit not found".into()); }
        Ok(())
    })
}

#[tauri::command]
pub fn add_quick_link(store: State<'_, Mutex<Store>>, label: String, url: String) -> Result<AppData, String> {
    let label = validate_title(&label)?;
    let url = validate_url(&url)?;
    with_store(store, |data| {
        if data.quick_links.len() >= 100 { return Err("Maximum 100 quick links".into()); }
        data.quick_links.push(QuickLink { id: new_id(), label, url, created_ms: now_ms() });
        Ok(())
    })
}

#[tauri::command]
pub fn remove_quick_link(store: State<'_, Mutex<Store>>, id: String) -> Result<AppData, String> {
    with_store(store, |data| {
        let before = data.quick_links.len();
        data.quick_links.retain(|item| item.id != id);
        if before == data.quick_links.len() { return Err("Quick link not found".into()); }
        Ok(())
    })
}

#[tauri::command]
pub fn open_quick_link(store: State<'_, Mutex<Store>>, id: String) -> Result<(), String> {
    let link = {
        let locked = store.lock().map_err(|_| "Storage lock poisoned")?;
        locked.data.quick_links.iter().find(|link| link.id == id)
            .ok_or("Quick link not found")?.url.clone()
    };
    let safe_url = validate_url(&link)?;
    #[cfg(windows)]
    {
        Command::new("explorer.exe").arg(&safe_url).spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = safe_url; Err("Quick launch is available only on Windows".into()) }
}

/// Fixed allowlist: never execute arbitrary user-supplied commands or shell expressions.
#[tauri::command]
pub fn launch_builtin(app_name: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let (program, arg) = match app_name.as_str() {
            "notepad" => ("notepad.exe", None),
            "calculator" => ("calc.exe", None),
            "explorer" => ("explorer.exe", None),
            "paint" => ("mspaint.exe", None),
            "settings" => ("explorer.exe", Some("ms-settings:")),
            _ => return Err("Unknown built-in Windows app".into()),
        };
        let mut cmd = Command::new(program);
        if let Some(arg) = arg { cmd.arg(arg); }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = app_name; Err("Quick launch is available only on Windows".into()) }
}

/// Sends standard Windows global media/volume virtual keys. It does not read track metadata.
#[tauri::command]
pub fn media_key(action: String) -> Result<(), String> {
    let code: u8 = match action.as_str() {
        "play_pause" => 0xB3,
        "next" => 0xB0,
        "previous" => 0xB1,
        "stop" => 0xB2,
        "mute" => 0xAD,
        "volume_down" => 0xAE,
        "volume_up" => 0xAF,
        _ => return Err("Unknown media action".into()),
    };
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn keybd_event(b_vk: u8, b_scan: u8, flags: u32, extra_info: usize);
        }
        // Compatibility path for Windows 10/11. Follow-up: migrate to SendInput.
        unsafe {
            keybd_event(code, 0, 0, 0);
            keybd_event(code, 0, 0x0002, 0);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = code; Err("Media keys are available only on Windows".into()) }
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[tauri::command]
pub fn get_autostart() -> Result<bool, String> {
    #[cfg(windows)]
    {
        let output = Command::new("reg").args(["query", RUN_KEY, "/v", "WFSNotch"])
            .output().map_err(|e| e.to_string())?;
        if !output.status.success() { return Ok(false); }
        Ok(String::from_utf8_lossy(&output.stdout).contains("WFSNotch"))
    }
    #[cfg(not(windows))]
    { Err("Autostart is available only on Windows".into()) }
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let output = if enabled {
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let quoted_path = format!("\"{}\"", executable.display());
            Command::new("reg")
                .args(["add", RUN_KEY, "/v", "WFSNotch", "/t", "REG_SZ", "/d", &quoted_path, "/f"])
                .output()
        } else {
            Command::new("reg").args(["delete", RUN_KEY, "/v", "WFSNotch", "/f"]).output()
        }.map_err(|e| e.to_string())?;
        if !output.status.success() && enabled {
            return Err(format!("Windows Run registry update failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        if !output.status.success() && !enabled && get_autostart()? {
            return Err("Failed to disable Windows autostart".into());
        }
        get_autostart()
    }
    #[cfg(not(windows))]
    { let _ = enabled; Err("Autostart is available only on Windows".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_calendar_days_and_urls() {
        assert!(validate_day("2028-02-29").is_ok());
        assert!(validate_day("2027-02-29").is_err());
        assert!(validate_day("2027-13-01").is_err());
        assert!(validate_url("https://example.com/path?q=1").is_ok());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("https://user:secret@example.com").is_err());
        assert!(validate_title("  work  ").is_ok());
        assert!(validate_title("   ").is_err());
    }
}
