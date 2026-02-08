use crate::config::Config;

#[allow(clippy::struct_field_names, reason = "fields mirror the domain terminology")]
pub struct Transition {
    config: Config,
    current_temperature: u16,
    target_temperature: u16,
    transition_start_temp: u16,
    transition_start_timestamp: u64,
    phase_start_time: std::time::Instant,
    in_transition: bool,
}

impl Transition {
    pub fn new_with_temp(config: Config, initial_temp: u16) -> Self {
        Self {
            config,
            current_temperature: initial_temp,
            target_temperature: initial_temp,
            transition_start_temp: initial_temp,
            transition_start_timestamp: current_unix_timestamp(),
            phase_start_time: std::time::Instant::now(),
            in_transition: false,
        }
    }

    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless,
        reason = "temperature values are small enough that casts between u16/i16/f64 are safe"
    )]
    pub fn update(&mut self, target_temp: u16) {
        let duration =
            std::time::Duration::from_secs(60 * u64::from(self.config.transition.duration_minutes));

        if duration.is_zero() {
            self.current_temperature = target_temp;
            self.target_temperature = target_temp;
            self.transition_start_temp = target_temp;
            self.transition_start_timestamp = current_unix_timestamp();
            self.in_transition = false;
            return;
        }

        if self.current_temperature == target_temp {
            self.target_temperature = target_temp;
            self.transition_start_temp = target_temp;
            self.in_transition = false;
            return;
        }

        if !self.in_transition || self.target_temperature != target_temp {
            self.transition_start_temp = self.current_temperature;
            self.target_temperature = target_temp;
            self.phase_start_time = std::time::Instant::now();
            self.transition_start_timestamp = current_unix_timestamp();
            self.in_transition = true;
        }

        let elapsed = self.phase_start_time.elapsed();

        if elapsed >= duration {
            self.current_temperature = self.target_temperature;
            self.in_transition = false;
            return;
        }

        let progress = elapsed.as_secs_f64() / duration.as_secs_f64();
        let eased_progress = self.apply_easing(progress);

        let temp_range = self.target_temperature as i16 - self.transition_start_temp as i16;
        let temp_delta = (temp_range as f64 * eased_progress) as i16;

        self.current_temperature = (self.transition_start_temp as i16 + temp_delta) as u16;
    }

    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless,
        reason = "temperature values are small enough that casts between u16/i16/f64 are safe"
    )]
    pub fn align_with_schedule(
        &mut self,
        start_temp: u16,
        target_temp: u16,
        elapsed: std::time::Duration,
    ) {
        let duration =
            std::time::Duration::from_secs(60 * u64::from(self.config.transition.duration_minutes));

        if duration.is_zero() {
            self.current_temperature = target_temp;
            self.target_temperature = target_temp;
            self.transition_start_temp = start_temp;
            self.transition_start_timestamp = current_unix_timestamp();
            self.phase_start_time = std::time::Instant::now();
            self.in_transition = false;
            return;
        }

        let clamped_elapsed = if elapsed > duration { duration } else { elapsed };
        let progress = clamped_elapsed.as_secs_f64() / duration.as_secs_f64();
        let eased_progress = self.apply_easing(progress);

        let temp_range = target_temp as i16 - start_temp as i16;
        let temp_delta = (temp_range as f64 * eased_progress) as i16;

        self.current_temperature = (start_temp as i16 + temp_delta) as u16;
        self.transition_start_temp = start_temp;
        self.target_temperature = target_temp;
        self.phase_start_time = std::time::Instant::now()
            .checked_sub(clamped_elapsed)
            .unwrap_or_else(std::time::Instant::now);
        self.transition_start_timestamp = current_unix_timestamp().saturating_sub(clamped_elapsed.as_secs());
        self.in_transition = clamped_elapsed < duration;
    }

    fn apply_easing(&self, t: f64) -> f64 {
        apply_easing(t, &self.config.transition.easing)
    }

    pub fn progress(&self) -> f64 {
        if !self.in_transition {
            return 1.0;
        }

        let elapsed = self.phase_start_time.elapsed();
        let duration =
            std::time::Duration::from_secs(60 * u64::from(self.config.transition.duration_minutes));

        if duration.is_zero() {
            return 1.0;
        }

        if elapsed >= duration {
            1.0
        } else {
            elapsed.as_secs_f64() / duration.as_secs_f64()
        }
    }

    pub const fn current_temperature(&self) -> u16 {
        self.current_temperature
    }

    pub const fn target_temperature(&self) -> u16 {
        self.target_temperature
    }

    pub const fn transition_start_temp(&self) -> u16 {
        self.transition_start_temp
    }

    pub const fn transition_start_timestamp(&self) -> u64 {
        self.transition_start_timestamp
    }
}

pub fn apply_easing(t: f64, easing: &str) -> f64 {
    match easing {
        "ease_in" => t * t,
        "ease_out" => t * (2.0 - t),
        "ease_in_out" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                2.0f64.mul_add(-t, 4.0).mul_add(t, -1.0)
            }
        }
        _ => t,
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::time::Duration;

    #[test]
    fn update_sets_progress_complete_when_at_target() {
        let config = Config::default();
        let mut transition = Transition::new_with_temp(config, 1500);

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 1500);
        assert_eq!(transition.progress(), 1.0);
        assert!(!transition.in_transition);
    }

    #[test]
    fn update_completes_after_elapsed_duration() {
        let mut config = Config::default();
        config.transition.duration_minutes = 1;
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.update(1500);
        transition.phase_start_time = std::time::Instant::now() - Duration::from_secs(60);
        transition.in_transition = true;

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 1500);
        assert_eq!(transition.progress(), 1.0);
        assert!(!transition.in_transition);
    }

    #[test]
    fn easing_linear_at_halfway() {
        let mut config = Config::default();
        config.transition.duration_minutes = 1;
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.update(1500);
        transition.phase_start_time = std::time::Instant::now() - Duration::from_secs(30);
        transition.in_transition = true;

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 4000);
    }

    #[test]
    fn easing_ease_in_at_halfway() {
        let mut config = Config::default();
        config.transition.duration_minutes = 1;
        config.transition.easing = "ease_in".to_string();
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.update(1500);
        transition.phase_start_time = std::time::Instant::now() - Duration::from_secs(30);
        transition.in_transition = true;

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 5250);
    }

    #[test]
    fn easing_ease_out_at_halfway() {
        let mut config = Config::default();
        config.transition.duration_minutes = 1;
        config.transition.easing = "ease_out".to_string();
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.update(1500);
        transition.phase_start_time = std::time::Instant::now() - Duration::from_secs(30);
        transition.in_transition = true;

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 2750);
    }

    #[test]
    fn easing_ease_in_out_at_halfway() {
        let mut config = Config::default();
        config.transition.duration_minutes = 1;
        config.transition.easing = "ease_in_out".to_string();
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.update(1500);
        transition.phase_start_time = std::time::Instant::now() - Duration::from_secs(30);
        transition.in_transition = true;

        transition.update(1500);

        assert_eq!(transition.current_temperature(), 4000);
    }

    #[test]
    fn align_with_schedule_sets_expected_temperature() {
        let mut config = Config::default();
        config.transition.duration_minutes = 60;
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.align_with_schedule(6500, 1500, Duration::from_secs(1800));

        assert_eq!(transition.current_temperature(), 4000);
    }
}
