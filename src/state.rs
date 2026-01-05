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
        let content = toml::to_string(self).unwrap_or_default();
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
    if path.starts_with("~") {
        dirs::home_dir().map(|home| home.join(&path[2..]))
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

    let temp_range = state.target_temp as i16 - state.transition_start_temp as i16;
    let temp_delta = (temp_range as f64 * eased_progress) as i16;

    (state.transition_start_temp as i16 + temp_delta) as u16
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
    use tempfile::TempDir;

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

    #[test]
    fn easing_applied_for_ease_out() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "ease_out");

        // ease_out at 0.5 progress -> eased 0.75
        assert_eq!(temp, 2750);
    }

    #[test]
    fn easing_applied_for_ease_in_out() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "ease_in_out");

        // ease_in_out at 0.5 progress -> eased 0.5 (linear at midpoint)
        assert_eq!(temp, 4000);
    }

    #[test]
    fn easing_unknown_defaults_to_linear() {
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 0,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let temp = calculate_temperature_from_state(&state, 3600, "unknown");

        // Unknown easing should default to linear
        assert_eq!(temp, 4000);
    }

    #[test]
    fn apply_easing_linear() {
        assert_eq!(apply_easing(0.0, "linear"), 0.0);
        assert_eq!(apply_easing(0.5, "linear"), 0.5);
        assert_eq!(apply_easing(1.0, "linear"), 1.0);
    }

    #[test]
    fn apply_easing_ease_in() {
        assert_eq!(apply_easing(0.0, "ease_in"), 0.0);
        assert_eq!(apply_easing(0.5, "ease_in"), 0.25);
        assert_eq!(apply_easing(1.0, "ease_in"), 1.0);
    }

    #[test]
    fn apply_easing_ease_out() {
        assert_eq!(apply_easing(0.0, "ease_out"), 0.0);
        assert_eq!(apply_easing(0.5, "ease_out"), 0.75);
        assert_eq!(apply_easing(1.0, "ease_out"), 1.0);
    }

    #[test]
    fn apply_easing_ease_in_out() {
        assert_eq!(apply_easing(0.0, "ease_in_out"), 0.0);
        assert_eq!(apply_easing(0.25, "ease_in_out"), 0.125);
        assert_eq!(apply_easing(0.5, "ease_in_out"), 0.5);
        assert_eq!(apply_easing(0.75, "ease_in_out"), 0.875);
        assert_eq!(apply_easing(1.0, "ease_in_out"), 1.0);
    }

    #[test]
    fn state_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.toml");
        
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 1234567890,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        state.save(state_path.to_str().unwrap()).unwrap();
        let loaded = State::load(state_path.to_str().unwrap()).unwrap();

        assert_eq!(loaded.transition_start_temp, 6500);
        assert_eq!(loaded.transition_start_timestamp, 1234567890);
        assert_eq!(loaded.elapsed_seconds, 1800);
        assert_eq!(loaded.target_temp, 1500);
    }

    #[test]
    fn state_load_nonexistent_returns_none() {
        let result = State::load("/nonexistent/path/state.toml");
        assert!(result.is_none());
    }

    #[test]
    fn state_age_seconds_calculates_correctly() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: now - 100,
            elapsed_seconds: 50,
            target_temp: 1500,
        };

        let age = state.age_seconds();
        
        // Age should be approximately the time since (timestamp + elapsed)
        // which is now - (now - 100 + 50) = 50 seconds
        assert!(age >= 49 && age <= 51);
    }

    #[test]
    fn expand_path_with_tilde() {
        let result = expand_path("~/test/path");
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(!path.to_string_lossy().contains("~"));
    }

    #[test]
    fn expand_path_without_tilde() {
        let result = expand_path("/absolute/path");
        assert_eq!(result, Some(std::path::PathBuf::from("/absolute/path")));
    }

    #[test]
    fn state_save_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("dir").join("state.toml");
        
        let state = State {
            transition_start_temp: 6500,
            transition_start_timestamp: 1234567890,
            elapsed_seconds: 1800,
            target_temp: 1500,
        };

        let result = state.save(nested_path.to_str().unwrap());
        assert!(result.is_ok());
        assert!(nested_path.exists());
    }
}
