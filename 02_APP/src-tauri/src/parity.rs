//! Parity-focused local health and countdown modules. No external network calls.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct HealthState {
    pub day: String,
    pub water_glasses: u16,
    pub break_count: u16,
    pub calm_minutes: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CountdownState {
    pub duration_seconds: u64,
    pub remaining_seconds: u64,
    pub ends_at_ms: Option<u128>,
    pub completed: bool,
}

impl Default for CountdownState {
    fn default() -> Self {
        Self {
            duration_seconds: 5 * 60,
            remaining_seconds: 5 * 60,
            ends_at_ms: None,
            completed: false,
        }
    }
}

impl CountdownState {
    pub fn normalize(&mut self, current: u128) {
        if let Some(end) = self.ends_at_ms {
            if end <= current {
                self.remaining_seconds = 0;
                self.ends_at_ms = None;
                self.completed = true;
            } else {
                self.remaining_seconds = (end - current).div_ceil(1000).min(86_400) as u64;
            }
        }
    }
    fn toggle(&mut self, current: u128) {
        self.normalize(current);
        if self.ends_at_ms.is_some() {
            self.ends_at_ms = None;
        } else {
            if self.remaining_seconds == 0 {
                self.remaining_seconds = self.duration_seconds;
            }
            self.ends_at_ms = Some(current.saturating_add(self.remaining_seconds as u128 * 1000));
            self.completed = false;
        }
    }
    fn adjust(&mut self, current: u128, minutes: u64) {
        self.normalize(current);
        let seconds = minutes * 60;
        self.remaining_seconds = self.remaining_seconds.saturating_add(seconds).min(86_400);
        self.duration_seconds = self.duration_seconds.max(self.remaining_seconds);
        if self.ends_at_ms.is_some() {
            self.ends_at_ms = Some(current.saturating_add(self.remaining_seconds as u128 * 1000));
        }
        self.completed = false;
    }
}

fn valid_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        && day.get(5..7).and_then(|v| v.parse::<u8>().ok()).is_some_and(|m| (1..=12).contains(&m))
        && day.get(8..10).and_then(|v| v.parse::<u8>().ok()).is_some_and(|d| (1..=31).contains(&d))
}

#[tauri::command]
pub fn health_action(
    store: State<'_, Mutex<Store>>,
    day: String,
    action: String,
) -> Result<AppData, String> {
    if !valid_day(&day) {
        return Err("Health date must be YYYY-MM-DD".into());
    }
    with_store(store, |data| {
        if data.health.day != day {
            data.health = HealthState {
                day,
                ..HealthState::default()
            };
        }
        match action.as_str() {
            "water_plus" => {
                data.health.water_glasses = data.health.water_glasses.saturating_add(1).min(40);
            }
            "water_minus" => {
                data.health.water_glasses = data.health.water_glasses.saturating_sub(1);
            }
            "break_plus" => {
                data.health.break_count = data.health.break_count.saturating_add(1).min(100);
            }
            "calm_plus" => {
                data.health.calm_minutes = data.health.calm_minutes.saturating_add(1).min(600);
            }
            _ => return Err("Unknown health action".into()),
        }
        Ok(())
    })
}

#[tauri::command]
pub fn countdown_set(
    store: State<'_, Mutex<Store>>,
    seconds: u64,
) -> Result<AppData, String> {
    if !(1..=86_400).contains(&seconds) {
        return Err("Countdown duration must be 1–86400 seconds".into());
    }
    with_store(store, |data| {
        data.countdown = CountdownState {
            duration_seconds: seconds,
            remaining_seconds: seconds,
            ..CountdownState::default()
        };
        Ok(())
    })
}

#[tauri::command]
pub fn countdown_toggle(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |data| {
        data.countdown.toggle(now_ms());
        Ok(())
    })
}

#[tauri::command]
pub fn countdown_adjust(
    store: State<'_, Mutex<Store>>,
    minutes: u64,
) -> Result<AppData, String> {
    if !matches!(minutes, 1 | 5) {
        return Err("Only +1 and +5 minute adjustments are supported".into());
    }
    with_store(store, |data| {
        data.countdown.adjust(now_ms(), minutes);
        Ok(())
    })
}

#[tauri::command]
pub fn countdown_reset(store: State<'_, Mutex<Store>>) -> Result<AppData, String> {
    with_store(store, |data| {
        data.countdown = CountdownState::default();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_date_validation() {
        assert!(valid_day("2026-11-07"));
        assert!(!valid_day("2026-13-07"));
        assert!(!valid_day("2026/11/07"));
        assert!(!valid_day("2026-11-00"));
    }

    #[test]
    fn countdown_lifecycle_and_suspend() {
        let mut timer = CountdownState::default();
        timer.toggle(10_000);
        assert_eq!(timer.ends_at_ms, Some(310_000));
        timer.normalize(70_000);
        assert_eq!(timer.remaining_seconds, 240);
        timer.adjust(70_000, 1);
        assert_eq!(timer.remaining_seconds, 300);
        timer.normalize(400_000);
        assert!(timer.completed);
        assert_eq!(timer.remaining_seconds, 0);
    }

    #[test]
    fn countdown_has_one_day_limit() {
        let mut timer = CountdownState::default();
        for _ in 0..400 {
            timer.adjust(0, 5);
        }
        assert_eq!(timer.remaining_seconds, 86_400);
    }
}
