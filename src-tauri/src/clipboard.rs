//! Clipboard values remain in the backend; the webview receives only an ownership token.
use crate::{AppError, AppResult};
use serde::Serialize;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
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

// Five retries after the first expiry attempt: at most 1.55s of backoff, never a copy replay.
const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

struct ScheduledClear {
    token: String,
    sequence: u32,
    deadline: Instant,
    retries: usize,
}

#[derive(Default)]
struct State {
    owned: Option<OwnedValue>,
    scheduled: Option<ScheduledClear>,
    worker_started: bool,
    shutdown: bool,
}

struct Inner {
    backend: Arc<dyn ClipboardBackend>,
    state: Mutex<State>,
    changed: Condvar,
    retry_delays: [Duration; 5],
}

impl Inner {
    fn start_worker(self: &Arc<Self>, state: &mut State) -> AppResult<()> {
        if !state.worker_started {
            let inner = self.clone();
            std::thread::Builder::new()
                .name("clipboard-clear".into())
                .spawn(move || inner.run())
                .map_err(|_| AppError::Storage)?;
            state.worker_started = true;
        }
        Ok(())
    }

    fn run(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        loop {
            if state.shutdown {
                state.worker_started = false;
                self.changed.notify_all();
                return;
            }
            let next = if let Some(scheduled) = &state.scheduled {
                let wait = scheduled.deadline.saturating_duration_since(Instant::now());
                if wait.is_zero() {
                    self.attempt_scheduled_clear(&mut state);
                    self.changed.notify_all();
                    continue;
                }
                self.changed
                    .wait_timeout(state, wait)
                    .map(|(state, _)| state)
                    .map_err(|_| ())
            } else {
                self.changed.wait(state).map_err(|_| ())
            };
            let Ok(next) = next else {
                return;
            };
            state = next;
        }
    }

    // The same mutex covers ownership checks, OS access and rescheduling. A new copy or
    // lock cannot leave a stale in-flight task able to clear a later clipboard value.
    fn attempt_scheduled_clear(&self, state: &mut State) {
        let Some(scheduled) = &state.scheduled else {
            return;
        };
        let Some(value) = state
            .owned
            .as_ref()
            .filter(|value| value.token == scheduled.token && value.sequence == scheduled.sequence)
        else {
            state.scheduled = None;
            return;
        };
        match self.backend.clear_if_matches(&value.text, value.sequence) {
            Ok(_) => {
                // A sequence/text mismatch is terminal too: the user owns that value now.
                state.owned = None;
                state.scheduled = None;
            }
            Err(_) => {
                let scheduled = state.scheduled.as_mut().expect("scheduled clear exists");
                if let Some(delay) = self.retry_delays.get(scheduled.retries) {
                    scheduled.retries += 1;
                    scheduled.deadline = Instant::now() + *delay;
                } else {
                    // Keep ownership for explicit lock/drop cleanup, but stop automatic attempts.
                    state.scheduled = None;
                }
            }
        }
    }
}

// The worker holds only Inner, not Owner, so the last service drop still clears immediately
// and wakes the worker to exit, even when its TTL is an hour away.
struct Owner {
    inner: Arc<Inner>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.shutdown = true;
            state.scheduled = None;
            if let Some(value) = state.owned.take() {
                let _ = self
                    .inner
                    .backend
                    .clear_if_matches(&value.text, value.sequence);
            }
            self.inner.changed.notify_all();
        }
    }
}

#[derive(Clone)]
pub struct ClipboardService(Arc<Owner>);

impl Default for ClipboardService {
    fn default() -> Self {
        Self::with_backend(Arc::new(SystemClipboard))
    }
}

impl ClipboardService {
    pub fn with_backend(backend: Arc<dyn ClipboardBackend>) -> Self {
        Self::with_retry_delays(backend, RETRY_DELAYS)
    }

    fn with_retry_delays(backend: Arc<dyn ClipboardBackend>, retry_delays: [Duration; 5]) -> Self {
        Self(Arc::new(Owner {
            inner: Arc::new(Inner {
                backend,
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
                retry_delays,
            }),
        }))
    }

    pub fn copy(&self, code: &str, ttl_secs: u32) -> AppResult<ClipboardToken> {
        if !(4..=10).contains(&code.len())
            || !code.bytes().all(|c| c.is_ascii_digit())
            || ttl_secs > 3600
        {
            return Err(AppError::InvalidInput);
        }
        self.copy_with_ttl(
            code,
            (ttl_secs > 0).then(|| Duration::from_secs(u64::from(ttl_secs))),
        )
    }

    fn copy_with_ttl(&self, code: &str, ttl: Option<Duration>) -> AppResult<ClipboardToken> {
        let inner = &self.0.inner;
        let mut state = inner.state.lock().map_err(|_| AppError::Storage)?;
        if ttl.is_some() {
            // Fail before writing if a worker cannot be started; never replay a clipboard write.
            inner.start_worker(&mut state)?;
        }
        let sequence = inner.backend.write(code)?;
        let token = uuid::Uuid::new_v4().to_string();
        state.owned = Some(OwnedValue {
            token: token.clone(),
            text: Zeroizing::new(code.to_owned()),
            sequence,
        });
        state.scheduled = ttl.map(|ttl| ScheduledClear {
            token: token.clone(),
            sequence,
            deadline: Instant::now() + ttl,
            retries: 0,
        });
        inner.changed.notify_all();
        Ok(ClipboardToken { token })
    }

    pub fn clear_token(&self, token: &str) -> AppResult<bool> {
        let inner = &self.0.inner;
        let mut state = inner.state.lock().map_err(|_| AppError::Storage)?;
        let Some(value) = state.owned.as_ref().filter(|v| v.token == token) else {
            return Ok(false);
        };
        let cleared = inner
            .backend
            .clear_if_matches(&value.text, value.sequence)?;
        state.owned = None;
        state.scheduled = None;
        inner.changed.notify_all();
        Ok(cleared)
    }

    /// Call on lock, idle-lock, and exit. Cancels scheduled clears even if immediate cleanup fails.
    pub fn clear_owned(&self) -> AppResult<bool> {
        let inner = &self.0.inner;
        let mut state = inner.state.lock().map_err(|_| AppError::Storage)?;
        state.scheduled = None;
        inner.changed.notify_all();
        let Some(value) = state.owned.as_ref() else {
            return Ok(false);
        };
        let cleared = inner
            .backend
            .clear_if_matches(&value.text, value.sequence)?;
        state.owned = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    #[derive(Default, Clone, Debug)]
    struct Contents {
        text: String,
        sequence: u32,
        writes: usize,
        attempts: usize,
        failures: usize,
    }

    #[derive(Default)]
    struct FailingClipboard(Mutex<Contents>);

    impl FailingClipboard {
        fn contents(&self) -> Contents {
            self.0.lock().unwrap().clone()
        }

        fn user_copy(&self, text: &str) {
            let mut contents = self.0.lock().unwrap();
            contents.text = text.into();
            contents.sequence += 1;
        }
    }

    impl ClipboardBackend for FailingClipboard {
        fn write(&self, text: &str) -> AppResult<u32> {
            let mut contents = self.0.lock().unwrap();
            contents.text = text.into();
            contents.sequence += 1;
            contents.writes += 1;
            Ok(contents.sequence)
        }

        fn clear_if_matches(&self, text: &str, sequence: u32) -> AppResult<bool> {
            let mut contents = self.0.lock().unwrap();
            contents.attempts += 1;
            if contents.failures > 0 {
                contents.failures -= 1;
                return Err(AppError::Storage);
            }
            if contents.text != text || contents.sequence != sequence {
                return Ok(false);
            }
            contents.text.clear();
            contents.sequence += 1;
            Ok(true)
        }
    }

    fn fixture(failures: usize, delay: Duration) -> (Arc<FailingClipboard>, ClipboardService) {
        let backend = Arc::new(FailingClipboard(Mutex::new(Contents {
            failures,
            ..Contents::default()
        })));
        let service = ClipboardService::with_retry_delays(backend.clone(), [delay; 5]);
        (backend, service)
    }

    // Event-driven waits have a failure deadline, not an assumed scheduler sleep duration.
    fn wait_for_state(inner: &Inner, predicate: impl Fn(&State) -> bool) -> MutexGuard<'_, State> {
        let state = inner.state.lock().unwrap();
        let (state, _) = inner
            .changed
            .wait_timeout_while(state, Duration::from_secs(3), |state| !predicate(state))
            .unwrap();
        assert!(
            predicate(&state),
            "clipboard scheduler did not reach expected state"
        );
        state
    }

    fn expire_now(service: &ClipboardService) -> ClipboardToken {
        service
            .copy_with_ttl("123456", Some(Duration::ZERO))
            .unwrap()
    }

    fn wait_for_first_failure(service: &ClipboardService) {
        drop(wait_for_state(&service.0.inner, |state| {
            state.scheduled.as_ref().is_some_and(|job| job.retries == 1)
        }));
    }

    #[test]
    fn expiry_recovers_after_one_or_several_failures_without_replaying_copy() {
        for failures in [1, 3] {
            let (backend, service) = fixture(failures, Duration::from_millis(1));
            expire_now(&service);
            drop(wait_for_state(&service.0.inner, |state| {
                state.owned.is_none()
            }));
            let contents = backend.contents();
            assert_eq!(contents.text, "");
            assert_eq!(contents.writes, 1);
            assert_eq!(contents.attempts, failures + 1);
        }
    }

    #[test]
    fn permanent_failure_stops_at_retry_limit_and_new_token_has_its_own_budget() {
        let (backend, service) = fixture(usize::MAX, Duration::from_millis(1));
        let old = expire_now(&service);
        {
            let state = wait_for_state(&service.0.inner, |state| state.scheduled.is_none());
            assert_eq!(state.owned.as_ref().unwrap().token, old.token);
        }
        assert_eq!(backend.contents().attempts, RETRY_DELAYS.len() + 1);
        assert_eq!(backend.contents().writes, 1);
        assert_eq!(backend.contents().text, "123456");

        // Reusing the same digits must still create a new token/sequence and fresh budget.
        backend.0.lock().unwrap().failures = 1;
        let new = expire_now(&service);
        assert_ne!(new.token, old.token);
        assert!(!service.clear_token(&old.token).unwrap());
        drop(wait_for_state(&service.0.inner, |state| {
            state.owned.is_none()
        }));
        let contents = backend.contents();
        assert_eq!(contents.attempts, RETRY_DELAYS.len() + 3);
        assert_eq!(contents.writes, 2);
        assert_eq!(contents.text, "");
    }

    #[test]
    fn new_copy_cancels_retry_including_when_the_digits_are_unchanged() {
        let (backend, service) = fixture(1, Duration::from_secs(60));
        let old = expire_now(&service);
        wait_for_first_failure(&service);
        let new = service.copy("123456", 0).unwrap();
        assert_ne!(new.token, old.token);
        assert!(!service.clear_token(&old.token).unwrap());
        {
            let state = service.0.inner.state.lock().unwrap();
            assert!(state.scheduled.is_none());
            assert_eq!(state.owned.as_ref().unwrap().token, new.token);
        }
        assert_eq!(backend.contents().attempts, 1);
        assert_eq!(backend.contents().writes, 2);
        assert_eq!(backend.contents().text, "123456");
        assert!(service.clear_token(&new.token).unwrap());
    }

    #[test]
    fn new_expiry_replaces_old_retry_and_does_not_wait_for_old_backoff() {
        let (backend, service) = fixture(1, Duration::from_secs(60));
        let old = expire_now(&service);
        wait_for_first_failure(&service);
        let new = expire_now(&service);
        assert_ne!(new.token, old.token);
        drop(wait_for_state(&service.0.inner, |state| {
            state.owned.is_none()
        }));
        assert_eq!(backend.contents().attempts, 2);
        assert_eq!(backend.contents().writes, 2);
        assert_eq!(backend.contents().text, "");
    }

    #[test]
    fn lock_cancels_retry_even_if_immediate_clear_also_fails() {
        let (backend, service) = fixture(usize::MAX, Duration::from_secs(60));
        expire_now(&service);
        wait_for_first_failure(&service);
        assert!(service.clear_owned().is_err());
        assert!(service.0.inner.state.lock().unwrap().scheduled.is_none());
        assert_eq!(backend.contents().attempts, 2);

        backend.user_copy("123456");
        backend.0.lock().unwrap().failures = 0;
        assert!(!service.clear_owned().unwrap());
        assert_eq!(backend.contents().text, "123456");
        assert_eq!(backend.contents().writes, 1);
    }

    #[test]
    fn user_copy_during_backoff_stops_retries_even_for_the_same_text() {
        for user_text in ["123456", "user notes"] {
            let (backend, service) = fixture(1, Duration::from_secs(60));
            expire_now(&service);
            wait_for_first_failure(&service);
            backend.user_copy(user_text);
            {
                let mut state = service.0.inner.state.lock().unwrap();
                state.scheduled.as_mut().unwrap().deadline = Instant::now();
                service.0.inner.changed.notify_all();
            }
            let state = wait_for_state(&service.0.inner, |state| state.owned.is_none());
            assert!(state.scheduled.is_none());
            assert_eq!(backend.contents().attempts, 2);
            assert_eq!(backend.contents().writes, 1);
            assert_eq!(backend.contents().text, user_text);
        }
    }

    #[test]
    fn stale_task_must_match_both_token_and_sequence_before_accessing_backend() {
        let (backend, service) = fixture(0, Duration::from_secs(60));
        let token = service.copy("123456", 0).unwrap();
        let inner = &service.0.inner;
        let mut state = inner.state.lock().unwrap();
        let sequence = state.owned.as_ref().unwrap().sequence;
        for (scheduled_token, scheduled_sequence) in [
            ("stale-token".to_owned(), sequence),
            (token.token.clone(), sequence + 1),
        ] {
            state.scheduled = Some(ScheduledClear {
                token: scheduled_token,
                sequence: scheduled_sequence,
                deadline: Instant::now(),
                retries: 0,
            });
            inner.attempt_scheduled_clear(&mut state);
            assert!(state.scheduled.is_none());
            assert_eq!(state.owned.as_ref().unwrap().token, token.token);
            assert_eq!(backend.contents().attempts, 0);
        }
    }

    #[test]
    fn explicit_clear_cancels_pending_retry() {
        let (backend, service) = fixture(1, Duration::from_secs(60));
        let token = expire_now(&service);
        wait_for_first_failure(&service);
        assert!(service.clear_token(&token.token).unwrap());
        let state = service.0.inner.state.lock().unwrap();
        assert!(state.scheduled.is_none());
        assert!(state.owned.is_none());
        assert_eq!(backend.contents().attempts, 2);
        assert_eq!(backend.contents().writes, 1);
    }

    #[test]
    fn final_service_drop_clears_and_stops_worker_waiting_in_backoff() {
        let (backend, service) = fixture(1, Duration::from_secs(60));
        expire_now(&service);
        wait_for_first_failure(&service);
        let inner = service.0.inner.clone();
        let clone = service.clone();
        drop(service);
        assert!(!inner.state.lock().unwrap().shutdown);
        assert_eq!(backend.contents().attempts, 1);
        drop(clone);
        let state = wait_for_state(&inner, |state| state.shutdown && !state.worker_started);
        assert!(state.scheduled.is_none());
        assert!(state.owned.is_none());
        assert_eq!(backend.contents().text, "");
        assert_eq!(backend.contents().attempts, 2);
        assert_eq!(backend.contents().writes, 1);
    }
}
