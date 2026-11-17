use std::time::{SystemTime, UNIX_EPOCH};

use crate::{AppError, AppResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeState {
    pub code_epoch: i64,
    pub remaining_secs: u32,
    pub progress: f64,
    pub clock_offset_secs: i64,
}

pub fn time_state(unix_seconds: i64, period_secs: u32, t0: i64) -> AppResult<TimeState> {
    time_state_with_offset(unix_seconds, period_secs, t0, 0)
}

pub fn time_state_with_offset(
    unix_seconds: i64,
    period_secs: u32,
    t0: i64,
    clock_offset_secs: i64,
) -> AppResult<TimeState> {
    if period_secs == 0 || unix_seconds < t0 {
        return Err(AppError::InvalidParameters);
    }
    let delta = i128::from(unix_seconds) - i128::from(t0);
    let period = i128::from(period_secs);
    let epoch = delta / period;
    let remainder = delta % period;
    let code_epoch = i128::from(t0) + epoch * period;
    let code_epoch = i64::try_from(code_epoch).map_err(|_| AppError::InvalidParameters)?;
    let remaining = u32::try_from(period - remainder).map_err(|_| AppError::InvalidParameters)?;
    let progress = (remainder as f64) / (period_secs as f64);
    Ok(TimeState {
        code_epoch,
        remaining_secs: remaining,
        progress,
        clock_offset_secs,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockSample {
    pub unix_seconds: i64,
    pub clock_offset_secs: i64,
    pub wall_clock_jump: bool,
}

#[derive(Debug)]
pub struct MonotonicClock {
    last_wall_seconds: Option<i64>,
    last_monotonic_nanos: Option<u128>,
    offset_secs: i64,
    monotonic_origin: std::time::Instant,
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self {
            last_wall_seconds: None,
            last_monotonic_nanos: None,
            offset_secs: 0,
            monotonic_origin: std::time::Instant::now(),
        }
    }
}

impl MonotonicClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, wall_seconds: i64, monotonic_nanos: u128) -> ClockSample {
        let Some(previous_wall) = self.last_wall_seconds else {
            self.last_wall_seconds = Some(wall_seconds);
            self.last_monotonic_nanos = Some(monotonic_nanos);
            return ClockSample {
                unix_seconds: wall_seconds,
                clock_offset_secs: self.offset_secs,
                wall_clock_jump: false,
            };
        };
        let previous_mono = self.last_monotonic_nanos.unwrap_or(monotonic_nanos);
        let monotonic_delta =
            ((monotonic_nanos.saturating_sub(previous_mono)) / 1_000_000_000) as i128;
        let expected_wall = i128::from(previous_wall) + monotonic_delta;
        let wall_delta = i128::from(wall_seconds) - expected_wall;
        let jumped = wall_delta.unsigned_abs() > 5;
        if jumped {
            self.offset_secs =
                self.offset_secs
                    .saturating_add(i64::try_from(wall_delta).unwrap_or_else(|_| {
                        if wall_delta.is_negative() {
                            i64::MIN
                        } else {
                            i64::MAX
                        }
                    }));
        }
        self.last_wall_seconds = Some(wall_seconds);
        self.last_monotonic_nanos = Some(monotonic_nanos);
        ClockSample {
            unix_seconds: wall_seconds.saturating_sub(self.offset_secs),
            clock_offset_secs: self.offset_secs,
            wall_clock_jump: jumped,
        }
    }

    pub fn sample_system(&mut self) -> AppResult<ClockSample> {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppError::InvalidParameters)?;
        let nanos = self.monotonic_origin.elapsed().as_nanos();
        Ok(self.sample(wall.as_secs() as i64, nanos))
    }

    pub fn time_state(
        &mut self,
        period_secs: u32,
        t0: i64,
        wall_seconds: i64,
        monotonic_nanos: u128,
    ) -> AppResult<TimeState> {
        let sample = self.sample(wall_seconds, monotonic_nanos);
        time_state_with_offset(
            sample.unix_seconds,
            period_secs,
            t0,
            sample.clock_offset_secs,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{MonotonicClock, time_state};

    #[test]
    fn reports_period_boundaries() {
        let first = time_state(59, 30, 0).unwrap();
        assert_eq!(first.code_epoch, 30);
        assert_eq!(first.remaining_secs, 1);
        assert!((first.progress - (29.0 / 30.0)).abs() < f64::EPSILON);

        let boundary = time_state(60, 30, 0).unwrap();
        assert_eq!(boundary.code_epoch, 60);
        assert_eq!(boundary.remaining_secs, 30);
        assert_eq!(boundary.progress, 0.0);
    }

    #[test]
    fn detects_and_compensates_wall_clock_jumps() {
        let mut clock = MonotonicClock::new();
        assert!(!clock.sample(100, 0).wall_clock_jump);
        assert!(!clock.sample(101, 1_000_000_000).wall_clock_jump);
        let jumped = clock.sample(120, 2_000_000_000);
        assert!(jumped.wall_clock_jump);
        assert_eq!(jumped.unix_seconds, 102);
        assert_eq!(jumped.clock_offset_secs, 18);
        let resumed = clock.sample(121, 3_000_000_000);
        assert!(!resumed.wall_clock_jump);
        assert_eq!(resumed.unix_seconds, 103);
    }

    #[test]
    fn handles_extreme_time_values_without_overflow() {
        let state = time_state(i64::MAX, 1, i64::MIN).unwrap();
        assert_eq!(state.code_epoch, i64::MAX);
        assert_eq!(state.remaining_secs, 1);
        assert!(time_state(i64::MIN, 30, i64::MAX).is_err());
    }
}
