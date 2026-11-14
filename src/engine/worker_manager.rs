//! Scale-to-Zero RPC Worker Manager (`pi-gpui/src/engine/worker_manager.rs`).
//!
//! Manages the lifecycle of the optional Node.js RPC worker so that it spawns only when
//! non-native extensions (`pi-subagents`, custom TS/JS extensions, or Node-only MCP servers)
//! are invoked, and automatically scales back to **0 MB** after `idle_ttl` (default 180s)
//! once `WorkerLivenessState::can_reclaim()` is true.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::client::sidecar::SidecarProcess;

/// Default idle timeout before reclaiming the Node.js RPC worker (180 seconds).
pub const DEFAULT_WORKER_IDLE_TTL: Duration = Duration::from_secs(180);

/// Tracks all active workloads on the Node.js RPC worker (`docs/gpui-migration-guide.md` §6.3.3).
///
/// The worker may ONLY be reclaimed when `can_reclaim()` returns `true` AND `idle_ttl` has elapsed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkerLivenessState {
    /// Active foreground LLM streaming turns running on the worker (`> 0` forbids reclaim).
    pub active_foreground_runs: usize,
    /// Active background subagent tasks (`pi-subagents` async / parallel / council workflows).
    pub active_subagent_runs: usize,
    /// Pending `extension_ui_request` dialogs waiting for user input (`questionnaire`, `confirm`, etc.).
    pub pending_ui_requests: usize,
    /// Active stateful MCP connections held open by the worker.
    pub active_mcp_sessions: usize,
}

impl WorkerLivenessState {
    /// Returns `true` if and only if every active counter is zero.
    pub fn can_reclaim(&self) -> bool {
        self.active_foreground_runs == 0
            && self.active_subagent_runs == 0
            && self.pending_ui_requests == 0
            && self.active_mcp_sessions == 0
    }
}

struct WorkerSlot {
    process: Option<SidecarProcess>,
    liveness: WorkerLivenessState,
    last_active_at: Instant,
    idle_ttl: Duration,
}

/// Thread-safe Scale-to-Zero manager for the Node.js RPC worker.
#[derive(Clone)]
pub struct ScaleToZeroWorkerManager {
    port: u16,
    token: String,
    instance_id: String,
    inner: Arc<Mutex<WorkerSlot>>,
}

impl ScaleToZeroWorkerManager {
    pub fn new(port: u16, token: String, instance_id: String) -> Self {
        Self {
            port,
            token,
            instance_id,
            inner: Arc::new(Mutex::new(WorkerSlot {
                process: None,
                liveness: WorkerLivenessState::default(),
                last_active_at: Instant::now(),
                idle_ttl: DEFAULT_WORKER_IDLE_TTL,
            })),
        }
    }

    /// Updates the current `WorkerLivenessState` snapshot from `AppState`.
    pub fn update_liveness(&self, liveness: WorkerLivenessState) {
        if let Ok(mut guard) = self.inner.lock() {
            if !liveness.can_reclaim() {
                guard.last_active_at = Instant::now();
            }
            guard.liveness = liveness;
        }
    }

    /// Returns a snapshot of the current `WorkerLivenessState`.
    pub fn liveness(&self) -> WorkerLivenessState {
        self.inner
            .lock()
            .map(|g| g.liveness.clone())
            .unwrap_or_default()
    }

    /// Marks activity on the worker, resetting its idle timer.
    pub fn touch(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.last_active_at = Instant::now();
        }
    }

    /// Spawns or attaches to the RPC worker on demand when an extension/subagent/MCP call needs it.
    pub fn ensure_started(&self) -> Result<String, String> {
        let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
        guard.last_active_at = Instant::now();
        if let Some(ref proc) = guard.process {
            return Ok(proc.base_url());
        }
        let proc = SidecarProcess::spawn_if_available(
            self.port,
            self.token.clone(),
            self.instance_id.clone(),
        )
        .ok_or_else(|| "Sidecar bundle not available for spawning".to_string())?;
        let url = proc.base_url();
        guard.process = Some(proc);
        Ok(url)
    }

    /// Checks whether the worker has been idle for `idle_ttl` with `can_reclaim() == true`,
    /// and if so, drops `SidecarProcess` (sending `SIGTERM` to reclaim Node.js memory to 0 MB).
    pub fn reclaim_if_idle(&self) -> bool {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        if guard.process.is_none() {
            return false;
        }
        if !guard.liveness.can_reclaim() {
            guard.last_active_at = Instant::now();
            return false;
        }
        if guard.last_active_at.elapsed() >= guard.idle_ttl {
            guard.process.take(); // Drop sends SIGTERM to spawned child
            return true;
        }
        false
    }

    /// Returns `true` if a managed worker process handle is currently held.
    pub fn is_worker_held(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.process.is_some())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_liveness_guards_reclaim() {
        let mut state = WorkerLivenessState::default();
        assert!(state.can_reclaim());

        state.active_subagent_runs = 1;
        assert!(!state.can_reclaim());

        state.active_subagent_runs = 0;
        state.pending_ui_requests = 1;
        assert!(!state.can_reclaim());

        state.pending_ui_requests = 0;
        assert!(state.can_reclaim());
    }
}
