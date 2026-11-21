use crate::{AppError, AppResult};
use std::{
    io::{Cursor, Read},
    path::Path,
};

pub const MAX_QR_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_QR_DIMENSION: u32 = 4096;

/// Decode only PNG/JPEG files, with compressed input and decoded dimensions bounded.
pub fn decode_qr_image(path: &Path) -> AppResult<Vec<String>> {
    let file = std::fs::File::open(path).map_err(|_| AppError::InvalidInput)?;
    let metadata = file.metadata().map_err(|_| AppError::InvalidInput)?;
    if !metadata.is_file() || metadata.len() > MAX_QR_FILE_BYTES {
        return Err(AppError::InvalidInput);
    }
    let mut bytes = Vec::new();
    file.take(MAX_QR_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::InvalidInput)?;
    if bytes.len() as u64 > MAX_QR_FILE_BYTES {
        return Err(AppError::InvalidInput);
    }
    decode_qr_bytes(&bytes)
}

pub fn decode_qr_bytes(bytes: &[u8]) -> AppResult<Vec<String>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_QR_FILE_BYTES {
        return Err(AppError::InvalidInput);
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AppError::InvalidInput)?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(AppError::InvalidInput);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_QR_DIMENSION);
    limits.max_image_height = Some(MAX_QR_DIMENSION);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let grayscale = reader
        .decode()
        .map_err(|_| AppError::InvalidInput)?
        .into_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(grayscale);
    let grids = prepared.detect_grids();
    if grids.is_empty() || grids.len() > 32 {
        return Err(AppError::InvalidInput);
    }
    let mut results = Vec::new();
    for grid in grids {
        if let Ok((_, content)) = grid.decode() {
            let value = content.trim();
            if !value.starts_with("otpauth://") && !value.starts_with("otpauth-migration://") {
                return Err(AppError::InvalidInput);
            }
            if !results.iter().any(|item| item == value) {
                results.push(value.to_owned());
            }
        }
    }
    if results.is_empty() {
        Err(AppError::InvalidInput)
    } else {
        Ok(results)
    }
}

#[cfg(feature = "desktop")]
pub fn pick_qr_image(locale: &str) -> AppResult<Option<Vec<String>>> {
    let labels = crate::commands::dialog_labels::for_locale(locale);
    let Some(path) = rfd::FileDialog::new()
        .add_filter(labels.qr_image, &["png", "jpg", "jpeg"])
        .pick_file()
    else {
        return Ok(None);
    };
    decode_qr_image(&path).map(Some)
}
