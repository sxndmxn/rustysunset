use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Auto,
    Fixed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
pub struct Transition {
    pub duration_minutes: u32,
    pub easing: String,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            duration_minutes: 60,
            easing: "smooth".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
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
#[serde(default)]
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
            status_file: "/tmp/candela.status".to_string(),
            optimize_updates: true,
            status_update_interval: 1,
            state_file: "~/.cache/candela/state.toml".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub location: Location,
    pub schedule: Schedule,
    pub transition: Transition,
    pub temperature: Temperature,
    pub daemon: Daemon,
}

pub fn find_config() -> Option<PathBuf> {
    let config_locations = [
        PathBuf::from("candela.toml"),
        dirs::config_dir()?.join("candela/config.toml"),
        dirs::config_dir()?.join("candela.toml"),
    ];

    config_locations.into_iter().find(|path| path.exists())
}

pub fn load(path: Option<&str>) -> Config {
    let mut config: Config = path.map_or_else(Config::default, |p| {
        let content = std::fs::read_to_string(p).unwrap_or_default();
        toml::from_str(&content).unwrap_or_else(|e| {
            log::warn!("Error parsing config: {e}");
            Config::default()
        })
    });

    // Apply defaults for any missing or empty daemon fields
    if config.daemon.tick_interval_seconds == 0 {
        config.daemon.tick_interval_seconds = 5;
    }
    if config.daemon.status_file.is_empty() {
        config.daemon.status_file = "/tmp/candela.status".to_string();
    }
    if config.daemon.state_file.is_empty() {
        config.daemon.state_file = "~/.cache/candela/state.toml".to_string();
    }

    apply_env(&mut config);
    config
}

fn apply_env(config: &mut Config) {
    if let Ok(val) = std::env::var("CANDELA_MODE") {
        match val.to_lowercase().as_str() {
            "auto" => config.mode = Mode::Auto,
            "fixed" => config.mode = Mode::Fixed,
            _ => {}
        }
    }

    if let Ok(val) = std::env::var("CANDELA_LATITUDE") {
        if let Ok(lat) = val.parse() {
            config.location.latitude = lat;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_LONGITUDE") {
        if let Ok(lon) = val.parse() {
            config.location.longitude = lon;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_DAY_TEMP") {
        if let Ok(temp) = val.parse() {
            config.temperature.day = temp;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_NIGHT_TEMP") {
        if let Ok(temp) = val.parse() {
            config.temperature.night = temp;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_TRANSITION_DURATION") {
        if let Ok(dur) = val.parse() {
            config.transition.duration_minutes = dur;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_EASING") {
        config.transition.easing = val;
    }

    if let Ok(val) = std::env::var("CANDELA_TICK_INTERVAL") {
        if let Ok(interval) = val.parse() {
            config.daemon.tick_interval_seconds = interval;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_STATUS_FILE") {
        config.daemon.status_file = val;
    }

    if let Ok(val) = std::env::var("CANDELA_WAKEUP") {
        config.schedule.wakeup = val;
    }

    if let Ok(val) = std::env::var("CANDELA_BEDTIME") {
        config.schedule.bedtime = val;
    }

    if let Ok(val) = std::env::var("CANDELA_OPTIMIZE_UPDATES") {
        config.daemon.optimize_updates = val.to_lowercase() != "false";
    }

    if let Ok(val) = std::env::var("CANDELA_STATUS_UPDATE_INTERVAL") {
        if let Ok(interval) = val.parse() {
            config.daemon.status_update_interval = interval;
        }
    }

    if let Ok(val) = std::env::var("CANDELA_STATE_FILE") {
        config.daemon.state_file = val;
    }
}
