//! Cancellation signals for running top-level requests.

use crate::domain::ports::{RequestControl, RequestState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Terminal states kept for late cancel calls (bounded).
const RECENT: usize = 1024;

// vhco:infra request_control satisfies RequestControl
/// (owner principal, cancellation signal, cancelling) per running request.
type Running = HashMap<String, (String, Arc<Notify>, bool)>;

#[derive(Default)]
pub struct RunningRequests {
    running: Mutex<Running>,
    recent: Mutex<Vec<(String, RequestState)>>,
}

impl RunningRequests {
    /// Register a request; the returned Notify fires on cancel.
    pub fn start(&self, request_id: &str, owner: &str) -> Arc<Notify> {
        let n = Arc::new(Notify::new());
        if let Ok(mut m) = self.running.lock() {
            m.insert(
                request_id.to_string(),
                (owner.to_string(), Arc::clone(&n), false),
            );
        }
        n
    }

    pub fn finish(&self, request_id: &str, state: &str) {
        let owner = self
            .running
            .lock()
            .ok()
            .and_then(|mut m| m.remove(request_id))
            .map(|(o, _, _)| o);
        if let (Some(owner), Ok(mut r)) = (owner, self.recent.lock()) {
            if r.len() >= RECENT {
                r.remove(0);
            }
            r.push((
                request_id.to_string(),
                RequestState {
                    owner,
                    state: state.to_string(),
                },
            ));
        }
    }
}

impl RequestControl for RunningRequests {
    fn lookup(&self, request_id: &str) -> Option<RequestState> {
        if let Some((owner, _, cancelling)) = self.running.lock().ok()?.get(request_id) {
            return Some(RequestState {
                owner: owner.clone(),
                state: if *cancelling { "cancelling" } else { "running" }.into(),
            });
        }
        self.recent
            .lock()
            .ok()?
            .iter()
            .rev()
            .find(|(id, _)| id == request_id)
            .map(|(_, s)| s.clone())
    }

    fn signal(&self, request_id: &str) -> bool {
        let Ok(mut m) = self.running.lock() else {
            return false;
        };
        match m.get_mut(request_id) {
            Some((_, n, cancelling)) => {
                *cancelling = true;
                n.notify_one();
                true
            }
            None => false,
        }
    }
}
