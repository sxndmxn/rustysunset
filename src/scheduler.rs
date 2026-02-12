use crate::config::{Config, Mode};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime, TimeZone};
use sunrise::{Coordinates, SolarDay, SolarEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Day,
    TransitioningToNight,
    Night,
    TransitioningToDay,
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::TransitioningToNight => "transitioning_to_night",
            Self::Night => "night",
            Self::TransitioningToDay => "transitioning_to_day",
        }
    }
}

pub struct TransitionWindow {
    pub start: DateTime<Local>,
    pub start_temp: u16,
    pub target_temp: u16,
}

pub struct Schedule {
    config: Config,
    wakeup_time: NaiveTime,
    bedtime_time: NaiveTime,
    coordinates: Coordinates,
}

impl Schedule {
    pub fn new(config: Config) -> Result<Self, String> {
        let wakeup_time = parse_time("wakeup", &config.schedule.wakeup)?;
        let bedtime_time = parse_time("bedtime", &config.schedule.bedtime)?;
        let coordinates = Coordinates::new(config.location.latitude, config.location.longitude)
            .ok_or_else(|| {
                format!(
                    "Invalid coordinates: latitude={} longitude={}",
                    config.location.latitude, config.location.longitude
                )
            })?;

        Ok(Self {
            config,
            wakeup_time,
            bedtime_time,
            coordinates,
        })
    }

    fn current_phase(&self) -> Phase {
        self.current_phase_at(Local::now())
    }

    pub fn current_phase_at(&self, now: DateTime<Local>) -> Phase {
        match self.config.mode {
            Mode::Auto => self.auto_phase(now),
            Mode::Fixed => self.fixed_phase(now),
        }
    }

    fn auto_phase(&self, now: DateTime<Local>) -> Phase {
        let (sunrise, sunset) = sunrise_sunset_local(&self.coordinates, now);
        let duration = Duration::minutes(i64::from(self.config.transition.duration_minutes));

        if now >= sunset + duration {
            Phase::Night
        } else if now >= sunset {
            Phase::TransitioningToNight
        } else if now >= sunrise + duration {
            Phase::Day
        } else if now >= sunrise {
            Phase::TransitioningToDay
        } else {
            Phase::Night
        }
    }

    fn fixed_phase(&self, now: DateTime<Local>) -> Phase {
        let now_time = now.time();

        let transition_duration = Duration::minutes(i64::from(self.config.transition.duration_minutes));
        let transition_start = self.bedtime_time - transition_duration;
        let transition_end = self.wakeup_time + transition_duration;

        if now_time >= self.wakeup_time && now_time < transition_end {
            Phase::TransitioningToDay
        } else if now_time >= transition_end && now_time < transition_start {
            Phase::Day
        } else if now_time >= transition_start && now_time < self.bedtime_time {
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

    pub fn transition_window_at(&self, now: DateTime<Local>) -> Option<TransitionWindow> {
        let duration = Duration::minutes(i64::from(self.config.transition.duration_minutes));
        if duration.is_zero() {
            return None;
        }

        match self.config.mode {
            Mode::Auto => self.auto_transition_window(now, duration),
            Mode::Fixed => self.fixed_transition_window(now, duration),
        }
    }

    fn auto_transition_window(
        &self,
        now: DateTime<Local>,
        duration: Duration,
    ) -> Option<TransitionWindow> {
        let (sunrise, sunset) = sunrise_sunset_local(&self.coordinates, now);

        if now >= sunset && now < sunset + duration {
            return Some(TransitionWindow {
                start: sunset,
                start_temp: self.config.temperature.day,
                target_temp: self.config.temperature.night,
            });
        }

        if now >= sunrise && now < sunrise + duration {
            return Some(TransitionWindow {
                start: sunrise,
                start_temp: self.config.temperature.night,
                target_temp: self.config.temperature.day,
            });
        }

        None
    }

    pub fn next_transition_start(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        match self.config.mode {
            Mode::Auto => self.auto_next_transition_start(now),
            Mode::Fixed => self.fixed_next_transition_start(now),
        }
    }

    fn auto_next_transition_start(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let (sunrise, sunset) = sunrise_sunset_local(&self.coordinates, now);
        let duration = Duration::minutes(i64::from(self.config.transition.duration_minutes));

        let phase = self.auto_phase(now);
        match phase {
            Phase::Day => Some(sunset),
            Phase::Night if now >= sunset + duration => {
                // Night after sunset — next transition is tomorrow's sunrise
                let tomorrow = now.date_naive().succ_opt()?;
                let tomorrow_noon = local_datetime(tomorrow, NaiveTime::from_hms_opt(12, 0, 0)?)?;
                let (tomorrow_sunrise, _) =
                    sunrise_sunset_local(&self.coordinates, tomorrow_noon);
                Some(tomorrow_sunrise)
            }
            Phase::Night => {
                // Night before sunrise — next transition is today's sunrise
                Some(sunrise)
            }
            Phase::TransitioningToNight | Phase::TransitioningToDay => None,
        }
    }

    fn fixed_next_transition_start(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let date = now.date_naive();
        let duration = Duration::minutes(i64::from(self.config.transition.duration_minutes));

        let phase = self.fixed_phase(now);
        match phase {
            Phase::Day => {
                // Next transition is bedtime - duration (start of TransitioningToNight)
                let bedtime_dt = local_datetime(date, self.bedtime_time)?;
                Some(bedtime_dt - duration)
            }
            Phase::Night if now.time() >= self.bedtime_time => {
                // Night after bedtime — next transition is tomorrow's wakeup
                let tomorrow = date.succ_opt()?;
                local_datetime(tomorrow, self.wakeup_time)
            }
            Phase::Night => {
                // Night before wakeup — next transition is today's wakeup
                local_datetime(date, self.wakeup_time)
            }
            Phase::TransitioningToNight | Phase::TransitioningToDay => None,
        }
    }

    fn fixed_transition_window(
        &self,
        now: DateTime<Local>,
        duration: Duration,
    ) -> Option<TransitionWindow> {
        let date = now.date_naive();
        let wakeup_dt = local_datetime(date, self.wakeup_time)?;
        let bedtime_dt = local_datetime(date, self.bedtime_time)?;

        let wakeup_end = wakeup_dt + duration;
        if now >= wakeup_dt && now < wakeup_end {
            return Some(TransitionWindow {
                start: wakeup_dt,
                start_temp: self.config.temperature.night,
                target_temp: self.config.temperature.day,
            });
        }

        let bedtime_start = bedtime_dt - duration;
        if now >= bedtime_start && now < bedtime_dt {
            return Some(TransitionWindow {
                start: bedtime_start,
                start_temp: self.config.temperature.day,
                target_temp: self.config.temperature.night,
            });
        }

        None
    }
}

fn parse_time(label: &str, value: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .map_err(|e| format!("Invalid {label} time '{value}': {e}"))
}

fn sunrise_sunset_local(coordinates: &Coordinates, now: DateTime<Local>) -> (DateTime<Local>, DateTime<Local>) {
    let solar_day = SolarDay::new(*coordinates, now.date_naive());

    let sunrise = solar_day
        .event_time(SolarEvent::Sunrise)
        .with_timezone(&Local);
    let sunset = solar_day
        .event_time(SolarEvent::Sunset)
        .with_timezone(&Local);

    (sunrise, sunset)
}

fn local_datetime(date: NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
    let naive = date.and_time(time);
    Local.from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .or_else(|| Local.from_local_datetime(&naive).latest())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Local, TimeZone, Timelike};

    /// Config with coordinates aligned to the local timezone so solar events
    /// fall on the expected local date regardless of where tests run.
    fn auto_test_config() -> Config {
        let mut config = Config::default();
        let offset_secs = Local::now().offset().local_minus_utc();
        let longitude = (f64::from(offset_secs) / 3600.0 * 15.0).clamp(-180.0, 180.0);
        config.location.latitude = 48.0;
        config.location.longitude = longitude;
        config
    }

    #[test]
    fn auto_phase_after_sunset_is_night() {
        let config = auto_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let after_sunset = sunset + Duration::hours(2);

        assert_eq!(schedule.current_phase_at(after_sunset), Phase::Night);
    }

    #[test]
    fn auto_phase_after_sunrise_is_transitioning_to_day() {
        let mut config = auto_test_config();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (sunrise, _) = sunrise_sunset_local(&schedule.coordinates, base);
        let half_transition = Duration::minutes(i64::from(config.transition.duration_minutes / 2));
        let during_transition = sunrise + half_transition;

        assert_eq!(
            schedule.current_phase_at(during_transition),
            Phase::TransitioningToDay
        );
    }

    #[test]
    fn auto_phase_midday_is_day() {
        let config = auto_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (sunrise, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let midpoint = sunrise + (sunset - sunrise) / 2;

        assert_eq!(schedule.current_phase_at(midpoint), Phase::Day);
    }

    #[test]
    fn fixed_schedule_rejects_invalid_time() {
        let mut config = Config::default();
        config.schedule.wakeup = "25:00".to_string();

        let result = Schedule::new(config);

        assert!(result.is_err());
    }

    #[test]
    fn auto_phase_at_sunset_is_transitioning_to_night() {
        let mut config = auto_test_config();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);

        assert_eq!(
            schedule.current_phase_at(sunset),
            Phase::TransitioningToNight
        );
    }

    #[test]
    fn auto_phase_at_transition_end_is_night() {
        let mut config = auto_test_config();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let end = sunset + Duration::minutes(i64::from(config.transition.duration_minutes));

        assert_eq!(schedule.current_phase_at(end), Phase::Night);
    }

    #[test]
    fn auto_phase_at_sunrise_is_transitioning_to_day() {
        let config = auto_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (sunrise, _) = sunrise_sunset_local(&schedule.coordinates, base);

        assert_eq!(
            schedule.current_phase_at(sunrise),
            Phase::TransitioningToDay
        );
    }

    #[test]
    fn fixed_phase_boundaries() {
        let mut config = Config::default();
        config.mode = Mode::Fixed;
        config.schedule.wakeup = "07:00".to_string();
        config.schedule.bedtime = "22:00".to_string();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config).expect("valid config");

        let day = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();

        let wakeup = day.with_hour(7).unwrap().with_minute(0).unwrap();
        let wakeup_end = wakeup + Duration::minutes(60);
        let bedtime = day.with_hour(22).unwrap().with_minute(0).unwrap();
        let bedtime_start = bedtime - Duration::minutes(60);

        assert_eq!(schedule.current_phase_at(wakeup), Phase::TransitioningToDay);
        assert_eq!(schedule.current_phase_at(wakeup_end), Phase::Day);
        assert_eq!(
            schedule.current_phase_at(bedtime_start),
            Phase::TransitioningToNight
        );
        assert_eq!(schedule.current_phase_at(bedtime), Phase::Night);
    }

    // --- next_transition_start tests (auto mode) ---

    #[test]
    fn next_transition_during_day_is_sunset() {
        let config = auto_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);

        let result = schedule.next_transition_start(base);
        assert_eq!(result, Some(sunset));
    }

    #[test]
    fn next_transition_during_night_after_sunset_is_tomorrow_sunrise() {
        let mut config = auto_test_config();
        config.transition.duration_minutes = 30;
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let night = sunset + Duration::hours(2);

        let tomorrow_noon = Local.with_ymd_and_hms(2024, 6, 2, 12, 0, 0).unwrap();
        let (tomorrow_sunrise, _) = sunrise_sunset_local(&schedule.coordinates, tomorrow_noon);

        let result = schedule.next_transition_start(night);
        assert_eq!(result, Some(tomorrow_sunrise));
    }

    #[test]
    fn next_transition_during_night_before_sunrise_is_today_sunrise() {
        let config = auto_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (sunrise, _) = sunrise_sunset_local(&schedule.coordinates, base);
        let early_morning = base.with_hour(2).unwrap().with_minute(0).unwrap();

        assert_eq!(schedule.current_phase_at(early_morning), Phase::Night);

        let result = schedule.next_transition_start(early_morning);
        assert_eq!(result, Some(sunrise));
    }

    #[test]
    fn next_transition_during_transition_is_none() {
        let mut config = auto_test_config();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let during = sunset + Duration::minutes(15);

        assert_eq!(
            schedule.current_phase_at(during),
            Phase::TransitioningToNight
        );
        assert_eq!(schedule.next_transition_start(during), None);
    }

    // --- next_transition_start tests (fixed mode) ---

    fn fixed_test_config() -> Config {
        let mut config = Config::default();
        config.mode = Mode::Fixed;
        config.schedule.wakeup = "07:00".to_string();
        config.schedule.bedtime = "22:00".to_string();
        config.transition.duration_minutes = 60;
        config
    }

    #[test]
    fn fixed_next_transition_during_day() {
        let config = fixed_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let day = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        assert_eq!(schedule.current_phase_at(day), Phase::Day);

        let expected = day.with_hour(21).unwrap().with_minute(0).unwrap();
        assert_eq!(schedule.next_transition_start(day), Some(expected));
    }

    #[test]
    fn fixed_next_transition_night_before_wakeup() {
        let config = fixed_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let early = Local.with_ymd_and_hms(2024, 6, 1, 3, 0, 0).unwrap();
        assert_eq!(schedule.current_phase_at(early), Phase::Night);

        let expected = early.with_hour(7).unwrap().with_minute(0).unwrap();
        assert_eq!(schedule.next_transition_start(early), Some(expected));
    }

    #[test]
    fn fixed_next_transition_night_after_bedtime() {
        let config = fixed_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let late = Local.with_ymd_and_hms(2024, 6, 1, 23, 0, 0).unwrap();
        assert_eq!(schedule.current_phase_at(late), Phase::Night);

        let expected = Local.with_ymd_and_hms(2024, 6, 2, 7, 0, 0).unwrap();
        assert_eq!(schedule.next_transition_start(late), Some(expected));
    }

    #[test]
    fn fixed_next_transition_during_transition_is_none() {
        let config = fixed_test_config();
        let schedule = Schedule::new(config).expect("valid config");

        let wakeup = Local.with_ymd_and_hms(2024, 6, 1, 7, 30, 0).unwrap();
        assert_eq!(
            schedule.current_phase_at(wakeup),
            Phase::TransitioningToDay
        );
        assert_eq!(schedule.next_transition_start(wakeup), None);
    }
}
