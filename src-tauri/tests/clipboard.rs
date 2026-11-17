use authenticator_desk_lib::AppResult;
use authenticator_desk_lib::clipboard::{ClipboardBackend, ClipboardService};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeClipboard(Mutex<(String, u32)>);
impl FakeClipboard {
    fn user_copy(&self, text: &str) {
        let mut c = self.0.lock().unwrap();
        c.0 = text.into();
        c.1 += 1;
    }
    fn text(&self) -> String {
        self.0.lock().unwrap().0.clone()
    }
}
impl ClipboardBackend for FakeClipboard {
    fn write(&self, text: &str) -> AppResult<u32> {
        self.user_copy(text);
        Ok(self.0.lock().unwrap().1)
    }
    fn clear_if_matches(&self, text: &str, sequence: u32) -> AppResult<bool> {
        let mut c = self.0.lock().unwrap();
        if c.0 == text && c.1 == sequence {
            c.0.clear();
            c.1 += 1;
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
    std::thread::sleep(std::time::Duration::from_millis(1150));
    assert_eq!(backend.text(), "");
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
