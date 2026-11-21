use authenticator_desk_lib::clipboard::{ClipboardBackend, ClipboardService};
use authenticator_desk_lib::{AppError, AppResult};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Contents {
    text: String,
    sequence: u32,
    writes: usize,
    clear_attempts: usize,
    clear_failures: usize,
}

#[derive(Default)]
struct FakeClipboard {
    contents: Mutex<Contents>,
    changed: Condvar,
}
impl FakeClipboard {
    fn user_copy(&self, text: &str) {
        let mut c = self.contents.lock().unwrap();
        c.text = text.into();
        c.sequence += 1;
    }
    fn text(&self) -> String {
        self.contents.lock().unwrap().text.clone()
    }
    fn wait_for_empty(&self) {
        let contents = self.contents.lock().unwrap();
        let (contents, _) = self
            .changed
            .wait_timeout_while(contents, Duration::from_secs(5), |c| !c.text.is_empty())
            .unwrap();
        assert!(
            contents.text.is_empty(),
            "scheduled clipboard clear did not finish"
        );
    }
}
impl ClipboardBackend for FakeClipboard {
    fn write(&self, text: &str) -> AppResult<u32> {
        let mut c = self.contents.lock().unwrap();
        c.text = text.into();
        c.sequence += 1;
        c.writes += 1;
        Ok(c.sequence)
    }
    fn clear_if_matches(&self, text: &str, sequence: u32) -> AppResult<bool> {
        let mut c = self.contents.lock().unwrap();
        c.clear_attempts += 1;
        if c.clear_failures > 0 {
            c.clear_failures -= 1;
            return Err(AppError::Storage);
        }
        if c.text == text && c.sequence == sequence {
            c.text.clear();
            c.sequence += 1;
            self.changed.notify_all();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
#[test]
fn clears_owned_value_but_preserves_later_user_copy_including_same_text() {
    let backend = Arc::new(FakeClipboard::default());
    let service = ClipboardService::with_backend(backend.clone());
    let token = service.copy("123456", 0).unwrap();
    assert!(service.clear_token(&token.token).unwrap());
    let token = service.copy("654321", 0).unwrap();
    backend.user_copy("654321");
    assert!(!service.clear_token(&token.token).unwrap());
    assert_eq!(backend.text(), "654321");
    let token = service.copy("123456", 0).unwrap();
    backend.user_copy("user notes");
    assert!(!service.clear_token(&token.token).unwrap());
    assert_eq!(backend.text(), "user notes");
}
#[test]
fn scheduled_clear_and_lock_cleanup_cancel_stale_tokens() {
    let backend = Arc::new(FakeClipboard::default());
    let service = ClipboardService::with_backend(backend.clone());
    let old = service.copy("111111", 0).unwrap();
    let new = service.copy("222222", 0).unwrap();
    assert!(!service.clear_token(&old.token).unwrap());
    assert_eq!(backend.text(), "222222");
    assert!(service.clear_owned().unwrap());
    assert!(!service.clear_token(&new.token).unwrap());
    service.copy("333333", 1).unwrap();
    backend.wait_for_empty();
    assert_eq!(backend.text(), "");
}
#[test]
fn expiry_retries_transient_failures_without_writing_the_code_again() {
    let backend = Arc::new(FakeClipboard::default());
    backend.contents.lock().unwrap().clear_failures = 3;
    let service = ClipboardService::with_backend(backend.clone());
    let token = service.copy("123456", 1).unwrap();
    backend.wait_for_empty();
    assert!(!service.clear_token(&token.token).unwrap());
    let contents = backend.contents.lock().unwrap();
    assert_eq!(contents.clear_attempts, 4);
    assert_eq!(contents.writes, 1);
}
#[test]
fn validates_codes_and_ttl_and_clears_on_last_service_drop() {
    let backend = Arc::new(FakeClipboard::default());
    {
        let service = ClipboardService::with_backend(backend.clone());
        assert!(service.copy("not-a-code", 30).is_err());
        assert!(service.copy("123456", 3601).is_err());
        service.copy("123456", 3600).unwrap();
        assert_eq!(backend.text(), "123456");
    }
    assert_eq!(backend.text(), "");
}
