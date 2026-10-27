use crate::contracts::{APP, ApiError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use uuid::Uuid;

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub struct MonotonicClock(Instant);
impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct Budget {
    pub total_ms: u64,
    pub child_ms: u64,
    pub max_concurrency: usize,
}

pub fn budgets() -> &'static HashMap<String, Budget> {
    static BUDGETS: std::sync::OnceLock<HashMap<String, Budget>> = std::sync::OnceLock::new();
    BUDGETS.get_or_init(|| {
        let schema: Value = serde_json::from_str(APP).expect("Embedded schema");
        serde_json::from_value(schema["x-request-budgets"]["operations"].clone())
            .expect("Budget table")
    })
}

#[derive(Default)]
struct Cancellation {
    cancelled: AtomicBool,
    stop_children: AtomicBool,
    parent: Option<Arc<Cancellation>>,
    source: Mutex<Option<String>>,
}
impl Cancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
            || self
                .parent
                .as_ref()
                .is_some_and(|p| p.is_cancelled() || p.stop_children.load(Ordering::SeqCst))
    }
}

#[derive(Default)]
pub struct Requests {
    active: Mutex<HashMap<Uuid, Arc<Cancellation>>>,
}

pub struct Job {
    id: Uuid,
    token: Arc<Cancellation>,
    requests: Arc<Requests>,
    clock: Arc<dyn Clock>,
    deadline: u64,
    pub budget: Budget,
}

impl Requests {
    pub fn begin(
        self: &Arc<Self>,
        id: &str,
        operation: &str,
        clock: Arc<dyn Clock>,
    ) -> Result<Job, ApiError> {
        let received_ms = clock.now_ms();
        self.begin_received(id, operation, clock, received_ms)
    }

    pub fn begin_received(
        self: &Arc<Self>,
        id: &str,
        operation: &str,
        clock: Arc<dyn Clock>,
        received_ms: u64,
    ) -> Result<Job, ApiError> {
        self.register(id, operation, clock, None, None, received_ms)
    }

    fn register(
        self: &Arc<Self>,
        id: &str,
        operation: &str,
        clock: Arc<dyn Clock>,
        parent: Option<Arc<Cancellation>>,
        parent_deadline: Option<u64>,
        received_ms: u64,
    ) -> Result<Job, ApiError> {
        let id = Uuid::parse_str(id).map_err(|_| ApiError::bad("request_id must be UUID"))?;
        let budget = *budgets()
            .get(operation)
            .ok_or_else(|| ApiError::bad("Unknown operation budget"))?;
        let deadline = received_ms
            .saturating_add(budget.total_ms)
            .min(parent_deadline.unwrap_or(u64::MAX));
        let mut active = self
            .active
            .lock()
            .map_err(|_| ApiError::internal("Request registry lock poisoned"))?;
        if active.contains_key(&id) {
            return Err(ApiError::bad("request_id is already active"));
        }
        let token = Arc::new(Cancellation {
            cancelled: AtomicBool::new(false),
            stop_children: AtomicBool::new(false),
            parent,
            source: Mutex::new(None),
        });
        active.insert(id, token.clone());
        Ok(Job {
            id,
            token,
            requests: self.clone(),
            clock,
            deadline,
            budget,
        })
    }

    pub fn cancel(&self, request_id: &str, target: &str) -> Result<Value, ApiError> {
        let id =
            Uuid::parse_str(request_id).map_err(|_| ApiError::bad("request_id must be UUID"))?;
        let target_id =
            Uuid::parse_str(target).map_err(|_| ApiError::bad("target_request_id must be UUID"))?;
        if id == target_id {
            return Err(ApiError::bad("Cancellation requires a distinct request_id"));
        }
        let active = self
            .active
            .lock()
            .map_err(|_| ApiError::internal("Request registry lock poisoned"))?;
        if active.contains_key(&id) {
            return Err(ApiError::bad("request_id is already active"));
        }
        let token = active.get(&target_id);
        if let Some(token) = token {
            token.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(serde_json::json!({"target_request_id": target, "cancelled": token.is_some()}))
    }

    pub fn active_count(&self) -> usize {
        self.active.lock().expect("Request registry").len()
    }

    pub fn cancel_sources(&self, sources: &[String]) {
        let active = self.active.lock().expect("Request registry");
        for token in active.values() {
            if token
                .source
                .lock()
                .expect("Source binding")
                .as_ref()
                .is_some_and(|id| sources.contains(id))
            {
                token.cancelled.store(true, Ordering::SeqCst);
            }
        }
    }
}

impl Job {
    pub fn bind_source(&self, source_id: &str) {
        *self.token.source.lock().expect("Source binding") = Some(source_id.into());
    }
    pub fn check(&self) -> Result<(), ApiError> {
        if self.token.is_cancelled() {
            return Err(ApiError::cancelled());
        }
        if self.clock.now_ms() >= self.deadline {
            return Err(ApiError::new(
                "UPSTREAM_TIMEOUT",
                "Operation deadline exhausted",
            ));
        }
        Ok(())
    }

    pub fn remaining_ms(&self) -> Result<u64, ApiError> {
        self.check()?;
        Ok(self.deadline.saturating_sub(self.clock.now_ms()))
    }

    pub fn child_timeout_ms(&self) -> Result<u64, ApiError> {
        Ok(self.remaining_ms()?.min(self.budget.child_ms))
    }
    pub fn connect_timeout_ms(&self) -> Result<u64, ApiError> {
        Ok(self.child_timeout_ms()?.min(10_000))
    }
    pub fn aggregate_remaining_ms(&self) -> Result<u64, ApiError> {
        let remaining = self.remaining_ms()?.saturating_sub(1_000);
        if remaining == 0 {
            self.token.stop_children.store(true, Ordering::SeqCst);
        }
        Ok(remaining)
    }
    pub fn child(&self, id: &str, operation: &str) -> Result<Job, ApiError> {
        self.check()?;
        if self.token.stop_children.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        let deadline = self
            .deadline
            .min(self.clock.now_ms().saturating_add(self.budget.child_ms));
        self.requests.register(
            id,
            operation,
            self.clock.clone(),
            Some(self.token.clone()),
            Some(deadline),
            self.clock.now_ms(),
        )
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // A finished parent also stops children that still own upstream work.
        self.token.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut active) = self.requests.active.lock() {
            active.remove(&self.id);
        }
    }
}
