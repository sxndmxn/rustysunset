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
        "linear" => t,
        "ease_in" => t * t,
        "ease_out" => t * (2.0 - t),
        "ease_in_out" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                2.0f64.mul_add(-t, 4.0).mul_add(t, -1.0)
            }
        }
        "sine" => (1.0 - (t * std::f64::consts::PI).cos()) / 2.0,
        "smooth" => t * t * 2.0f64.mul_add(-t, 3.0),
        "smoother" => t * t * t * t.mul_add(6.0f64.mul_add(t, -15.0), 10.0),
        _ => parse_cubic_bezier(easing)
            .map_or(t, |[x1, y1, x2, y2]| eval_cubic_bezier(t, x1, y1, x2, y2)),
    }
}

fn parse_cubic_bezier(s: &str) -> Option<[f64; 4]> {
    let inner = s.trim().strip_prefix("cubic_bezier(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some([
        parts[0].trim().parse().ok()?,
        parts[1].trim().parse().ok()?,
        parts[2].trim().parse().ok()?,
        parts[3].trim().parse().ok()?,
    ])
}

#[allow(
    clippy::similar_names,
    reason = "ax/bx/cx/ay/by/cy are standard polynomial coefficient names for bezier curves"
)]
fn eval_cubic_bezier(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let cx = 3.0 * x1;
    let bx = 3.0f64.mul_add(x2 - x1, -cx);
    let ax = 1.0 - cx - bx;

    let cy = 3.0 * y1;
    let by = 3.0f64.mul_add(y2 - y1, -cy);
    let ay = 1.0 - cy - by;

    // Newton's method: find t where x(t) = x
    let mut t = x;
    for _ in 0..8 {
        let x_t = ax.mul_add(t, bx).mul_add(t, cx) * t;
        let dx = (3.0 * ax).mul_add(t, 2.0 * bx).mul_add(t, cx);
        if dx.abs() < 1e-12 {
            break;
        }
        t -= (x_t - x) / dx;
    }
    t = t.clamp(0.0, 1.0);

    ay.mul_add(t, by).mul_add(t, cy) * t
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
        config.transition.easing = "linear".to_string();
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
        config.transition.easing = "linear".to_string();
        let mut transition = Transition::new_with_temp(config, 6500);

        transition.align_with_schedule(6500, 1500, Duration::from_secs(1800));

        assert_eq!(transition.current_temperature(), 4000);
    }

    #[test]
    fn easing_sine_boundaries() {
        assert!(apply_easing(0.0, "sine").abs() < f64::EPSILON);
        assert!((apply_easing(1.0, "sine") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_smooth_boundaries() {
        assert!(apply_easing(0.0, "smooth").abs() < f64::EPSILON);
        assert!((apply_easing(1.0, "smooth") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_smoother_boundaries() {
        assert!(apply_easing(0.0, "smoother").abs() < f64::EPSILON);
        assert!((apply_easing(1.0, "smoother") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_sine_at_midpoint() {
        assert!((apply_easing(0.5, "sine") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_smooth_at_midpoint() {
        assert!((apply_easing(0.5, "smooth") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_smoother_at_midpoint() {
        assert!((apply_easing(0.5, "smoother") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_curves_differ_at_quarter() {
        let sine = apply_easing(0.25, "sine");
        let smooth = apply_easing(0.25, "smooth");
        let smoother = apply_easing(0.25, "smoother");
        let linear = apply_easing(0.25, "linear");

        assert!((sine - smooth).abs() > 0.001);
        assert!((sine - smoother).abs() > 0.001);
        assert!((smooth - smoother).abs() > 0.001);
        assert!((sine - linear).abs() > 0.001);
    }

    #[test]
    fn cubic_bezier_linear_equivalent() {
        let result = apply_easing(0.5, "cubic_bezier(0.0, 0.0, 1.0, 1.0)");
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn cubic_bezier_css_ease() {
        let result = apply_easing(0.5, "cubic_bezier(0.25, 0.1, 0.25, 1.0)");
        assert!(result > 0.0 && result < 1.0);
    }

    #[test]
    fn cubic_bezier_invalid_fallback() {
        assert!((apply_easing(0.5, "cubic_bezier(invalid)") - 0.5).abs() < f64::EPSILON);
        assert!((apply_easing(0.5, "not_a_curve") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cubic_bezier_endpoints() {
        let result_0 = apply_easing(0.0, "cubic_bezier(0.25, 0.1, 0.25, 1.0)");
        let result_1 = apply_easing(1.0, "cubic_bezier(0.25, 0.1, 0.25, 1.0)");
        assert!(result_0.abs() < 0.01);
        assert!((result_1 - 1.0).abs() < 0.01);
    }
}
