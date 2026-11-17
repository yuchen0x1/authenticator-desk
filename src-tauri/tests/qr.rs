use authenticator_desk_lib::qr::decode_qr_image;
#[test]
fn decodes_real_standard_uri_image_and_rejects_blank_or_invalid_files() {
    let dir = std::env::temp_dir().join(format!("authdesk-qr-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let uri = "otpauth://totp/RFC:test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    let code = qrcode::QrCode::new(uri).unwrap();
    let bitmap = code
        .render::<image::Luma<u8>>()
        .min_dimensions(512, 512)
        .build();
    let file = dir.join("fixture.png");
    bitmap.save(&file).unwrap();
    assert_eq!(decode_qr_image(&file).unwrap(), vec![uri]);
    let blank = dir.join("blank.png");
    image::GrayImage::from_pixel(200, 200, image::Luma([255]))
        .save(&blank)
        .unwrap();
    assert!(decode_qr_image(&blank).is_err());
    let invalid = dir.join("invalid.png");
    std::fs::write(&invalid, b"not an image").unwrap();
    assert!(decode_qr_image(&invalid).is_err());
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn rejects_oversized_input_dimensions_and_non_otp_payload() {
    use authenticator_desk_lib::qr::{MAX_QR_DIMENSION, MAX_QR_FILE_BYTES, decode_qr_bytes};
    assert!(decode_qr_bytes(&[]).is_err());
    assert!(decode_qr_bytes(&vec![0; MAX_QR_FILE_BYTES as usize + 1]).is_err());
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::GrayImage::from_pixel(MAX_QR_DIMENSION + 1, 1, image::Luma([255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    assert!(decode_qr_bytes(bytes.get_ref()).is_err());
    let qr = qrcode::QrCode::new("https://example.test/not-an-otp").unwrap();
    let bitmap = qr
        .render::<image::Luma<u8>>()
        .min_dimensions(256, 256)
        .build();
    let mut bytes = std::io::Cursor::new(Vec::new());
    bitmap
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    assert!(decode_qr_bytes(bytes.get_ref()).is_err());
}
