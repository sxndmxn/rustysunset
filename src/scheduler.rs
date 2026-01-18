use crate::config::{Config, Mode};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime, TimeZone};
use sunrise::{Coordinates, SolarDay, SolarEvent};

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

    pub fn current_phase(&self) -> Phase {
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

        let transition_duration = Duration::minutes(self.config.transition.duration_minutes as i64);
        let next_sunrise = sunrise + Duration::days(1);

        if now >= sunset && now < next_sunrise - transition_duration {
            Phase::Night
        } else if now >= next_sunrise - transition_duration && now < next_sunrise {
            Phase::TransitioningToDay
        } else if now >= sunrise && now < sunrise + transition_duration {
            Phase::TransitioningToDay
        } else if now >= sunrise + transition_duration && now < sunset - transition_duration {
            Phase::Day
        } else if now >= sunset - transition_duration && now < sunset {
            Phase::TransitioningToNight
        } else if now < sunrise - transition_duration {
            Phase::Night
        } else {
            Phase::TransitioningToDay
        }
    }

    fn fixed_phase(&self, now: DateTime<Local>) -> Phase {
        let now_time = now.time();

        let transition_duration = Duration::minutes(self.config.transition.duration_minutes as i64);
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
        let duration = Duration::minutes(self.config.transition.duration_minutes as i64);
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

        let to_night_start = sunset - duration;
        if now >= to_night_start && now < sunset {
            return Some(TransitionWindow {
                start: to_night_start,
                start_temp: self.config.temperature.day,
                target_temp: self.config.temperature.night,
            });
        }

        let to_day_start = sunrise - duration;
        if now >= to_day_start && now < sunrise {
            return Some(TransitionWindow {
                start: to_day_start,
                start_temp: self.config.temperature.night,
                target_temp: self.config.temperature.day,
            });
        }

        None
    }

    fn fixed_transition_window(
        &self,
        now: DateTime<Local>,
        duration: Duration,
    ) -> Option<TransitionWindow> {
        let date = now.date_naive();
        let wakeup_dt = local_datetime(date, self.wakeup_time);
        let bedtime_dt = local_datetime(date, self.bedtime_time);

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
        .map_err(|e| format!("Invalid {} time '{}': {}", label, value, e))
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

fn local_datetime(date: NaiveDate, time: NaiveTime) -> DateTime<Local> {
    let naive = date.and_time(time);
    Local.from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .or_else(|| Local.from_local_datetime(&naive).latest())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Local, TimeZone, Timelike};

    #[test]
    fn auto_phase_after_sunset_is_night() {
        let config = Config::default();
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let after_sunset = sunset + Duration::hours(2);

        assert_eq!(schedule.current_phase_at(after_sunset), Phase::Night);
    }

    #[test]
    fn auto_phase_before_next_sunrise_is_transitioning_to_day() {
        let mut config = Config::default();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (sunrise, _) = sunrise_sunset_local(&schedule.coordinates, base);
        let next_sunrise = sunrise + Duration::days(1);
        let half_transition = Duration::minutes((config.transition.duration_minutes / 2) as i64);
        let before_next_sunrise = next_sunrise - half_transition;

        assert_eq!(
            schedule.current_phase_at(before_next_sunrise),
            Phase::TransitioningToDay
        );
    }

    #[test]
    fn auto_phase_midday_is_day() {
        let config = Config::default();
        let schedule = Schedule::new(config.clone()).expect("valid config");

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
    fn auto_phase_at_transition_start_to_night() {
        let mut config = Config::default();
        config.transition.duration_minutes = 60;
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);
        let start = sunset - Duration::minutes(config.transition.duration_minutes as i64);

        assert_eq!(
            schedule.current_phase_at(start),
            Phase::TransitioningToNight
        );
    }

    #[test]
    fn auto_phase_at_sunset_is_night() {
        let config = Config::default();
        let schedule = Schedule::new(config.clone()).expect("valid config");

        let base = Local.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let (_, sunset) = sunrise_sunset_local(&schedule.coordinates, base);

        assert_eq!(schedule.current_phase_at(sunset), Phase::Night);
    }

    #[test]
    fn auto_phase_at_sunrise_is_transitioning_to_day() {
        let config = Config::default();
        let schedule = Schedule::new(config.clone()).expect("valid config");

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
}
