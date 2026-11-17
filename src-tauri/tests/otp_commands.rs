use authenticator_desk_lib::otp::{
    OtpAlgorithm, OtpConfig, OtpKind, SecretBytes, generate_code, time_state,
};

#[test]
fn public_otp_api_is_available_without_tauri_runtime() {
    let config = OtpConfig {
        secret: SecretBytes::from_base32("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap(),
        algorithm: OtpAlgorithm::Sha1,
        digits: 6,
        kind: OtpKind::Totp {
            period_secs: 30,
            t0: 0,
        },
    };
    assert_eq!(generate_code(&config, 59).unwrap(), "287082");
    assert_eq!(time_state(59, 30, 0).unwrap().remaining_secs, 1);
}
