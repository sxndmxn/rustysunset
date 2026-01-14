use crate::config::Config;

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

    pub fn update(&mut self, target_temp: u16) {
        let duration =
            std::time::Duration::from_secs(60 * self.config.transition.duration_minutes as u64);

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

        let temp_range = self.target_temperature as i32 - self.transition_start_temp as i32;
        let temp_delta = (temp_range as f64 * eased_progress) as i32;
        let result = (self.transition_start_temp as i32 + temp_delta).clamp(0, u16::MAX as i32);

        self.current_temperature = result as u16;
    }

    fn apply_easing(&self, t: f64) -> f64 {
        match self.config.transition.easing.as_str() {
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

    pub fn progress(&self) -> f64 {
        if !self.in_transition {
            return 1.0;
        }

        let elapsed = self.phase_start_time.elapsed();
        let duration =
            std::time::Duration::from_secs(60 * self.config.transition.duration_minutes as u64);

        if duration.is_zero() {
            return 1.0;
        }

        if elapsed >= duration {
            1.0
        } else {
            elapsed.as_secs_f64() / duration.as_secs_f64()
        }
    }

    pub fn current_temperature(&self) -> u16 {
        self.current_temperature
    }

    pub fn target_temperature(&self) -> u16 {
        self.target_temperature
    }

    pub fn transition_start_temp(&self) -> u16 {
        self.transition_start_temp
    }

    pub fn transition_start_timestamp(&self) -> u64 {
        self.transition_start_timestamp
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
}
