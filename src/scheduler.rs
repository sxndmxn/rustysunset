use crate::config::{Config, Mode};
use chrono::{Datelike, Duration, Local, NaiveTime, TimeZone};
use sunrise::sunrise_sunset;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Day,
    TransitioningToNight,
    Night,
    TransitioningToDay,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Day => "day",
            Phase::TransitioningToNight => "transitioning_to_night",
            Phase::Night => "night",
            Phase::TransitioningToDay => "transitioning_to_day",
        }
    }
}

pub struct Schedule {
    config: Config,
}

impl Schedule {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn current_phase(&self) -> Phase {
        match self.config.mode {
            Mode::Auto => self.auto_phase(),
            Mode::Fixed => self.fixed_phase(),
        }
    }

    fn auto_phase(&self) -> Phase {
        let now = Local::now();
        let (sunrise_ts, sunset_ts) = sunrise_sunset(
            self.config.location.latitude,
            self.config.location.longitude,
            now.year(),
            now.month(),
            now.day(),
        );

        let sunrise = Local.timestamp_opt(sunrise_ts, 0).unwrap();
        let sunset = Local.timestamp_opt(sunset_ts, 0).unwrap();

        let now_time = now.time();
        let sunrise_time = sunrise.time();
        let sunset_time = sunset.time();

        let transition_duration = Duration::minutes(self.config.transition.duration_minutes as i64);
        let transition_start = sunset_time - transition_duration;
        let transition_end = sunrise_time + transition_duration;

        if now_time >= transition_start && now_time < sunset_time {
            Phase::TransitioningToNight
        } else if now_time >= sunset_time && now_time < transition_start {
            Phase::Night
        } else if now_time >= sunrise_time && now_time < transition_end {
            Phase::TransitioningToDay
        } else if now_time >= transition_end && now_time < transition_start {
            Phase::Day
        } else if now_time < sunrise_time && now_time >= transition_start {
            Phase::Night
        } else {
            Phase::Day
        }
    }

    fn fixed_phase(&self) -> Phase {
        let now = Local::now();
        let now_time = now.time();

        let wakeup_time = NaiveTime::parse_from_str(&self.config.schedule.wakeup, "%H:%M").unwrap();
        let bedtime_time =
            NaiveTime::parse_from_str(&self.config.schedule.bedtime, "%H:%M").unwrap();

        let transition_duration = Duration::minutes(self.config.transition.duration_minutes as i64);
        let transition_start = bedtime_time - transition_duration;
        let transition_end = wakeup_time + transition_duration;

        if now_time >= wakeup_time && now_time < transition_end {
            Phase::TransitioningToDay
        } else if now_time >= transition_end && now_time < transition_start {
            Phase::Day
        } else if now_time >= transition_start && now_time < bedtime_time {
            Phase::TransitioningToNight
        } else {
            Phase::Night
        }
    }

    pub fn target_temperature(&self) -> u16 {
        let phase = self.current_phase();
        match phase {
            Phase::Day | Phase::TransitioningToDay => self.config.temperature.day,
            Phase::Night | Phase::TransitioningToNight => self.config.temperature.night,
        }
    }
}
