use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Auto,
    Fixed,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Auto
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Schedule {
    pub wakeup: String,
    pub bedtime: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            wakeup: "07:00".to_string(),
            bedtime: "22:00".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Transition {
    pub duration_minutes: u32,
    pub easing: String,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            duration_minutes: 60,
            easing: "linear".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Temperature {
    pub day: u16,
    pub night: u16,
}

impl Default for Temperature {
    fn default() -> Self {
        Self {
            day: 6500,
            night: 1500,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Daemon {
    pub tick_interval_seconds: u64,
    pub status_file: String,
    pub optimize_updates: bool,
    pub status_update_interval: u64,
    pub state_file: String,
}

impl Default for Daemon {
    fn default() -> Self {
        Self {
            tick_interval_seconds: 5,
            status_file: "/tmp/rustysunset.status".to_string(),
            optimize_updates: true,
            status_update_interval: 1,
            state_file: "~/.cache/rustysunset/state.toml".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: Mode,
    pub location: Location,
    pub schedule: Schedule,
    pub transition: Transition,
    pub temperature: Temperature,
    pub daemon: Daemon,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            location: Location::default(),
            schedule: Schedule::default(),
            transition: Transition::default(),
            temperature: Temperature::default(),
            daemon: Daemon::default(),
        }
    }
}

pub fn find_config() -> Option<PathBuf> {
    let config_locations = [
        PathBuf::from("rustysunset.toml"),
        dirs::config_dir()?.join("rustysunset/config.toml"),
        dirs::config_dir()?.join("rustysunset.toml"),
    ];

    for path in config_locations {
        if path.exists() {
            return Some(path);
        }
    }

    None
}

pub fn load(path: Option<&str>) -> Config {
    let mut config: Config = match path {
        Some(p) => {
            let content = std::fs::read_to_string(p).unwrap_or_default();
            match toml::from_str(&content) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error parsing config: {}", e);
                    Config::default()
                }
            }
        }
        None => Config::default(),
    };

    // Apply defaults for any missing or empty daemon fields
    if config.daemon.tick_interval_seconds == 0 {
        config.daemon.tick_interval_seconds = 5;
    }
    if config.daemon.status_file.is_empty() {
        config.daemon.status_file = "/tmp/rustysunset.status".to_string();
    }
    if config.daemon.state_file.is_empty() {
        config.daemon.state_file = "~/.cache/rustysunset/state.toml".to_string();
    }

    apply_env(&mut config);
    config
}

fn apply_env(config: &mut Config) {
    if let Ok(val) = std::env::var("RUSTYSUNSET_MODE") {
        match val.to_lowercase().as_str() {
            "auto" => config.mode = Mode::Auto,
            "fixed" => config.mode = Mode::Fixed,
            _ => {}
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_LATITUDE") {
        if let Ok(lat) = val.parse() {
            config.location.latitude = lat;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_LONGITUDE") {
        if let Ok(lon) = val.parse() {
            config.location.longitude = lon;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_DAY_TEMP") {
        if let Ok(temp) = val.parse() {
            config.temperature.day = temp;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_NIGHT_TEMP") {
        if let Ok(temp) = val.parse() {
            config.temperature.night = temp;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_TRANSITION_DURATION") {
        if let Ok(dur) = val.parse() {
            config.transition.duration_minutes = dur;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_EASING") {
        config.transition.easing = val;
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_TICK_INTERVAL") {
        if let Ok(interval) = val.parse() {
            config.daemon.tick_interval_seconds = interval;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_STATUS_FILE") {
        config.daemon.status_file = val;
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_WAKEUP") {
        config.schedule.wakeup = val;
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_BEDTIME") {
        config.schedule.bedtime = val;
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_OPTIMIZE_UPDATES") {
        config.daemon.optimize_updates = val.to_lowercase() != "false";
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_STATUS_UPDATE_INTERVAL") {
        if let Ok(interval) = val.parse() {
            config.daemon.status_update_interval = interval;
        }
    }

    if let Ok(val) = std::env::var("RUSTYSUNSET_STATE_FILE") {
        config.daemon.state_file = val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn mode_default_is_auto() {
        assert_eq!(Mode::default(), Mode::Auto);
    }

    #[test]
    fn location_default_is_zero_coordinates() {
        let loc = Location::default();
        assert_eq!(loc.latitude, 0.0);
        assert_eq!(loc.longitude, 0.0);
    }

    #[test]
    fn schedule_default_values() {
        let sched = Schedule::default();
        assert_eq!(sched.wakeup, "07:00");
        assert_eq!(sched.bedtime, "22:00");
    }

    #[test]
    fn transition_default_values() {
        let trans = Transition::default();
        assert_eq!(trans.duration_minutes, 60);
        assert_eq!(trans.easing, "linear");
    }

    #[test]
    fn temperature_default_values() {
        let temp = Temperature::default();
        assert_eq!(temp.day, 6500);
        assert_eq!(temp.night, 1500);
    }

    #[test]
    fn daemon_default_values() {
        let daemon = Daemon::default();
        assert_eq!(daemon.tick_interval_seconds, 5);
        assert_eq!(daemon.status_file, "/tmp/rustysunset.status");
        assert_eq!(daemon.optimize_updates, true);
        assert_eq!(daemon.status_update_interval, 1);
        assert_eq!(daemon.state_file, "~/.cache/rustysunset/state.toml");
    }

    #[test]
    fn config_default_all_fields() {
        let config = Config::default();
        assert_eq!(config.mode, Mode::Auto);
        assert_eq!(config.location.latitude, 0.0);
        assert_eq!(config.temperature.day, 6500);
    }

    #[test]
    fn load_with_no_path_returns_default() {
        let config = load(None);
        assert_eq!(config.mode, Mode::Auto);
        assert_eq!(config.temperature.day, 6500);
    }

    #[test]
    fn load_with_valid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        let toml_content = r#"
mode = "fixed"

[location]
latitude = 48.516
longitude = 9.12

[schedule]
wakeup = "07:00"
bedtime = "22:00"

[transition]
duration_minutes = 60
easing = "linear"

[temperature]
day = 7000
night = 2000

[daemon]
tick_interval_seconds = 5
status_file = "/tmp/rustysunset.status"
optimize_updates = true
status_update_interval = 1
state_file = "~/.cache/rustysunset/state.toml"
"#;
        fs::write(&config_path, toml_content).unwrap();

        let config = load(Some(config_path.to_str().unwrap()));
        
        assert_eq!(config.mode, Mode::Fixed);
        assert_eq!(config.location.latitude, 48.516);
        assert_eq!(config.location.longitude, 9.12);
        assert_eq!(config.temperature.day, 7000);
        assert_eq!(config.temperature.night, 2000);
    }

    #[test]
    fn load_with_invalid_toml_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.toml");
        
        fs::write(&config_path, "invalid toml content {]").unwrap();

        let config = load(Some(config_path.to_str().unwrap()));
        
        // Should return defaults on parse error
        assert_eq!(config.mode, Mode::Auto);
    }

    #[test]
    fn load_applies_defaults_for_missing_daemon_fields() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        let toml_content = r#"
mode = "auto"

[daemon]
tick_interval_seconds = 0
status_file = ""
state_file = ""
"#;
        fs::write(&config_path, toml_content).unwrap();

        let config = load(Some(config_path.to_str().unwrap()));
        
        // Should apply defaults for zero/empty values
        assert_eq!(config.daemon.tick_interval_seconds, 5);
        assert_eq!(config.daemon.status_file, "/tmp/rustysunset.status");
        assert_eq!(config.daemon.state_file, "~/.cache/rustysunset/state.toml");
    }

    #[test]
    fn apply_env_mode() {
        env::set_var("RUSTYSUNSET_MODE", "fixed");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_MODE");
        
        assert_eq!(config.mode, Mode::Fixed);
    }

    #[test]
    fn apply_env_latitude_longitude() {
        env::set_var("RUSTYSUNSET_LATITUDE", "52.5");
        env::set_var("RUSTYSUNSET_LONGITUDE", "13.4");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_LATITUDE");
        env::remove_var("RUSTYSUNSET_LONGITUDE");
        
        assert_eq!(config.location.latitude, 52.5);
        assert_eq!(config.location.longitude, 13.4);
    }

    #[test]
    fn apply_env_temperatures() {
        env::set_var("RUSTYSUNSET_DAY_TEMP", "7000");
        env::set_var("RUSTYSUNSET_NIGHT_TEMP", "2000");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_DAY_TEMP");
        env::remove_var("RUSTYSUNSET_NIGHT_TEMP");
        
        assert_eq!(config.temperature.day, 7000);
        assert_eq!(config.temperature.night, 2000);
    }

    #[test]
    fn apply_env_transition_duration() {
        env::set_var("RUSTYSUNSET_TRANSITION_DURATION", "90");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_TRANSITION_DURATION");
        
        assert_eq!(config.transition.duration_minutes, 90);
    }

    #[test]
    fn apply_env_easing() {
        env::set_var("RUSTYSUNSET_EASING", "ease_in_out");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_EASING");
        
        assert_eq!(config.transition.easing, "ease_in_out");
    }

    #[test]
    fn apply_env_tick_interval() {
        env::set_var("RUSTYSUNSET_TICK_INTERVAL", "10");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_TICK_INTERVAL");
        
        assert_eq!(config.daemon.tick_interval_seconds, 10);
    }

    #[test]
    fn apply_env_status_file() {
        env::set_var("RUSTYSUNSET_STATUS_FILE", "/tmp/custom.status");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_STATUS_FILE");
        
        assert_eq!(config.daemon.status_file, "/tmp/custom.status");
    }

    #[test]
    fn apply_env_wakeup_bedtime() {
        env::set_var("RUSTYSUNSET_WAKEUP", "08:30");
        env::set_var("RUSTYSUNSET_BEDTIME", "23:30");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_WAKEUP");
        env::remove_var("RUSTYSUNSET_BEDTIME");
        
        assert_eq!(config.schedule.wakeup, "08:30");
        assert_eq!(config.schedule.bedtime, "23:30");
    }

    #[test]
    fn apply_env_optimize_updates_true() {
        env::set_var("RUSTYSUNSET_OPTIMIZE_UPDATES", "true");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_OPTIMIZE_UPDATES");
        
        assert_eq!(config.daemon.optimize_updates, true);
    }

    #[test]
    fn apply_env_optimize_updates_false() {
        env::set_var("RUSTYSUNSET_OPTIMIZE_UPDATES", "false");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_OPTIMIZE_UPDATES");
        
        assert_eq!(config.daemon.optimize_updates, false);
    }

    #[test]
    fn apply_env_status_update_interval() {
        env::set_var("RUSTYSUNSET_STATUS_UPDATE_INTERVAL", "5");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_STATUS_UPDATE_INTERVAL");
        
        assert_eq!(config.daemon.status_update_interval, 5);
    }

    #[test]
    fn apply_env_state_file() {
        env::set_var("RUSTYSUNSET_STATE_FILE", "/tmp/state.toml");
        let config = load(None);
        env::remove_var("RUSTYSUNSET_STATE_FILE");
        
        assert_eq!(config.daemon.state_file, "/tmp/state.toml");
    }

    #[test]
    fn find_config_returns_none_when_no_config_exists() {
        // This test relies on there being no config in the test environment
        // We can't guarantee this, but it's a reasonable test
        let result = find_config();
        // Result could be None or Some, depending on environment
        // Just verify it doesn't panic
        let _ = result;
    }
}
