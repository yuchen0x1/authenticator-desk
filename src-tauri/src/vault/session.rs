use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::{AppError, AppResult};

#[derive(Clone)]
pub struct SessionHandle {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    key: Mutex<Option<Zeroizing<[u8; 32]>>>,
    generation: u64,
    invalidated: AtomicBool,
    revision: AtomicU64,
}

impl SessionHandle {
    pub(crate) fn new(key: Zeroizing<[u8; 32]>, generation: u64) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                key: Mutex::new(Some(key)),
                generation,
                invalidated: AtomicBool::new(false),
                revision: AtomicU64::new(0),
            }),
        }
    }

    pub fn generation(&self) -> u64 {
        self.inner.generation
    }

    pub fn is_invalidated(&self) -> bool {
        self.inner.invalidated.load(Ordering::Acquire)
    }

    #[allow(dead_code)]
    pub(crate) fn key_copy(&self) -> AppResult<Zeroizing<[u8; 32]>> {
        self.inner
            .key
            .lock()
            .map_err(|_| AppError::Storage)?
            .as_ref()
            .cloned()
            .ok_or(AppError::VaultLocked)
    }

    pub(crate) fn invalidate(&self) -> AppResult<()> {
        let mut key = self.inner.key.lock().map_err(|_| AppError::Storage)?;
        *key = None;
        self.inner.invalidated.store(true, Ordering::Release);
        self.inner.revision.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.inner.revision.load(Ordering::Acquire)
    }
}

pub struct IdleState {
    timeout: Duration,
    last_activity: Instant,
}

impl IdleState {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last_activity: Instant::now(),
        }
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
        self.last_activity = Instant::now();
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn expired(&self) -> bool {
        !self.timeout.is_zero() && self.last_activity.elapsed() >= self.timeout
    }
}
