use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct State {
    pub transition_start_temp: u16,
    pub transition_start_timestamp: u64,
    pub elapsed_seconds: u64,
    pub target_temp: u16,
}

impl State {
    pub fn load(path: &str) -> Option<Self> {
        let path = expand_path(path)?;
        let content = fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let path = expand_path(path)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid path"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        fs::write(&path, content)
    }

    pub fn age_seconds(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.transition_start_timestamp + self.elapsed_seconds)
    }
}

fn expand_path(path: &str) -> Option<PathBuf> {
    if path == "~" {
        dirs::home_dir()
    } else if let Some(rest) = path.strip_prefix("~/") {
        dirs::home_dir().map(|home| home.join(rest))
    } else {
        Some(PathBuf::from(path))
    }
}

pub fn calculate_temperature_from_state(
    state: &State,
    transition_duration_seconds: u64,
    easing: &str,
) -> u16 {
    if state.elapsed_seconds >= transition_duration_seconds {
        return state.target_temp;
    }

    let progress = state.elapsed_seconds as f64 / transition_duration_seconds as f64;
    let eased_progress = apply_easing(progress, easing);

    let temp_range = state.target_temp as i32 - state.transition_start_temp as i32;
    let temp_delta = (temp_range as f64 * eased_progress) as i32;
    let result = (state.transition_start_temp as i32 + temp_delta).clamp(0, u16::MAX as i32);

    result as u16
}

fn apply_easing(t: f64, easing: &str) -> f64 {
    match easing {
        "ease_in" => t * t,
        "ease_out" => t * (2.0 - t),
        "ease_in_out" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        _ => t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_temperature_uses_saved_state_mid_transition() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "linear");

        assert_eq!(temp, 4000);
    }

    #[test]
    fn calculate_temperature_returns_target_when_elapsed_exceeds_duration() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 4000,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "linear");

        assert_eq!(temp, 1500);
    }

    #[test]
    fn easing_applied_for_ease_in() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "ease_in");

        // ease_in at 0.5 progress -> eased 0.25
        assert_eq!(temp, 5250);
    }
}
