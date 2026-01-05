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
