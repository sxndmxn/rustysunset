use crate::config::Config;

pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

pub struct Transition {
    config: Config,
    current_temperature: u16,
    target_temperature: u16,
    transition_start_temp: u16,
    phase_start_time: std::time::Instant,
    in_transition: bool,
}

impl Transition {
    pub fn new(config: Config) -> Self {
        let current_temp = config.temperature.day;
        Self {
            config,
            current_temperature: current_temp,
            target_temperature: current_temp,
            transition_start_temp: current_temp,
            phase_start_time: std::time::Instant::now(),
            in_transition: false,
        }
    }

    pub fn update(&mut self, target_temp: u16) {
        let elapsed = self.phase_start_time.elapsed();
        let duration =
            std::time::Duration::from_secs(60 * self.config.transition.duration_minutes as u64);

        if !self.in_transition {
            self.transition_start_temp = self.current_temperature;
            self.target_temperature = target_temp;
            self.phase_start_time = std::time::Instant::now();
            self.in_transition = true;
        }

        if elapsed < duration {
            let progress = elapsed.as_secs_f64() / duration.as_secs_f64();
            let eased_progress = self.apply_easing(progress);

            let temp_range = self.target_temperature as i16 - self.transition_start_temp as i16;
            let temp_delta = (temp_range as f64 * eased_progress) as i16;

            self.current_temperature = (self.transition_start_temp as i16 + temp_delta) as u16;
        } else {
            self.current_temperature = self.target_temperature;
            self.in_transition = false;
        }
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

    pub fn set_immediate(&mut self, temp: u16) {
        self.current_temperature = temp;
        self.transition_start_temp = temp;
        self.target_temperature = temp;
        self.phase_start_time = std::time::Instant::now();
        self.in_transition = false;
    }
}
