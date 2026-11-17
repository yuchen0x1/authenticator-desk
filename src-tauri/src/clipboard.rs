//! Clipboard values remain in the backend; the webview receives only an ownership token.
use crate::{AppError, AppResult};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub trait ClipboardBackend: Send + Sync + 'static {
    fn write(&self, text: &str) -> AppResult<u32>;
    /// Check ownership and clear atomically while holding the OS clipboard open.
    fn clear_if_matches(&self, text: &str, sequence: u32) -> AppResult<bool>;
}

#[derive(Clone, Debug, Serialize)]
pub struct ClipboardToken {
    pub token: String,
}

struct OwnedValue {
    token: String,
    text: Zeroizing<String>,
    sequence: u32,
}

struct Inner {
    backend: Arc<dyn ClipboardBackend>,
    owned: Mutex<Option<OwnedValue>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(Some(value)) = self.owned.get_mut().map(Option::take) {
            let _ = self.backend.clear_if_matches(&value.text, value.sequence);
        }
    }
}

#[derive(Clone)]
pub struct ClipboardService(Arc<Inner>);

impl Default for ClipboardService {
    fn default() -> Self {
        Self::with_backend(Arc::new(SystemClipboard))
    }
}

impl ClipboardService {
    pub fn with_backend(backend: Arc<dyn ClipboardBackend>) -> Self {
        Self(Arc::new(Inner {
            backend,
            owned: Mutex::new(None),
        }))
    }

    pub fn copy(&self, code: &str, ttl_secs: u32) -> AppResult<ClipboardToken> {
        if !(4..=10).contains(&code.len())
            || !code.bytes().all(|c| c.is_ascii_digit())
            || ttl_secs > 3600
        {
            return Err(AppError::InvalidInput);
        }
        let mut owned = self.0.owned.lock().map_err(|_| AppError::Storage)?;
        let sequence = self.0.backend.write(code)?;
        let token = uuid::Uuid::new_v4().to_string();
        *owned = Some(OwnedValue {
            token: token.clone(),
            text: Zeroizing::new(code.to_owned()),
            sequence,
        });
        drop(owned);
        if ttl_secs > 0 {
            let weak = Arc::downgrade(&self.0);
            let scheduled_token = token.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(u64::from(ttl_secs)));
                if let Some(inner) = weak.upgrade() {
                    let _ = ClipboardService(inner).clear_token(&scheduled_token);
                }
            });
        }
        Ok(ClipboardToken { token })
    }

    pub fn clear_token(&self, token: &str) -> AppResult<bool> {
        let mut owned = self.0.owned.lock().map_err(|_| AppError::Storage)?;
        let Some(value) = owned.as_ref().filter(|v| v.token == token) else {
            return Ok(false);
        };
        let cleared = self
            .0
            .backend
            .clear_if_matches(&value.text, value.sequence)?;
        *owned = None;
        Ok(cleared)
    }

    /// Call on lock, idle-lock, and exit. Invalidates any previously scheduled tokens.
    pub fn clear_owned(&self) -> AppResult<bool> {
        let mut owned = self.0.owned.lock().map_err(|_| AppError::Storage)?;
        let Some(value) = owned.as_ref() else {
            return Ok(false);
        };
        let cleared = self
            .0
            .backend
            .clear_if_matches(&value.text, value.sequence)?;
        *owned = None;
        Ok(cleared)
    }
}

struct SystemClipboard;

#[cfg(windows)]
impl ClipboardBackend for SystemClipboard {
    fn write(&self, text: &str) -> AppResult<u32> {
        use clipboard_win::{Clipboard, Setter, formats};
        let _clipboard = Clipboard::new_attempts(10).map_err(|_| AppError::Storage)?;
        formats::Unicode
            .write_clipboard(&text)
            .map_err(|_| AppError::Storage)?;
        clipboard_win::seq_num()
            .map(|n| n.get())
            .ok_or(AppError::Storage)
    }
    fn clear_if_matches(&self, text: &str, sequence: u32) -> AppResult<bool> {
        use clipboard_win::{Clipboard, Getter, formats};
        let _clipboard = Clipboard::new_attempts(10).map_err(|_| AppError::Storage)?;
        if clipboard_win::seq_num().map(|n| n.get()) != Some(sequence) {
            return Ok(false);
        }
        let mut current = Zeroizing::new(String::new());
        if formats::Unicode.read_clipboard(&mut *current).is_err() || *current != text {
            return Ok(false);
        }
        clipboard_win::empty().map_err(|_| AppError::Storage)?;
        Ok(true)
    }
}

#[cfg(not(windows))]
impl ClipboardBackend for SystemClipboard {
    fn write(&self, _: &str) -> AppResult<u32> {
        Err(AppError::InvalidParameters)
    }
    fn clear_if_matches(&self, _: &str, _: u32) -> AppResult<bool> {
        Ok(false)
    }
}
