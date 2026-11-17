use super::{OtpConfig, hotp::generate_hotp};
use crate::{AppError, AppResult};

pub(crate) fn generate_totp(
    config: &OtpConfig,
    unix_seconds: i64,
    period_secs: u32,
    t0: i64,
) -> AppResult<String> {
    if period_secs == 0 || unix_seconds < t0 {
        return Err(AppError::InvalidParameters);
    }
    let delta = i128::from(unix_seconds) - i128::from(t0);
    let counter =
        u64::try_from(delta / i128::from(period_secs)).map_err(|_| AppError::InvalidParameters)?;
    generate_hotp(config, counter)
}
