//! Local core discovery for Settings › General › Pi Core.
//!
//! The desktop app talks to an HTTP core. Two cores can serve it:
//! - `pi` (pi-web / this app's own API sidecar): default `http://127.0.0.1:30141`
//!   for a standalone pi-web, `38471` for the bundled API sidecar.
//! - `omp` (oh-my-pi): the CLI speaks JSON-RPC over stdio (`omp --mode rpc`), so it
//!   exposes no coding-agent HTTP API. Its documented default *local HTTP* listener
//!   is the stats dashboard on `http://127.0.0.1:3847` (`port = 3847`,
//!   `STATS_DASHBOARD_HOSTNAME = 127.0.0.1` in `packages/stats/src/`), which is what
//!   auto-detection probes.
//!
//! Probing is a short TCP connect: no request is sent, nothing is spawned.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// One candidate address and what answered there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreCandidate {
    /// Stable id: `pi-web`, `pi-sidecar`, or `omp`.
    pub id: &'static str,
    pub label: &'static str,
    pub url: String,
    pub found: bool,
}

/// Ports probed by `Auto-detect`, in priority order.
pub const CORE_CANDIDATES: &[(&str, &str, u16)] = &[
    // Bundled API sidecar (this app's own core).
    ("pi-sidecar", "Pi Agent sidecar", 38471),
    // pi-web's documented default listen address.
    ("pi-web", "pi / pi-web", 30141),
    // omp's documented default local HTTP listener (stats dashboard).
    ("omp", "omp (oh-my-stats)", 3847),
];

pub fn probe_candidates() -> Vec<CoreCandidate> {
    CORE_CANDIDATES
        .iter()
        .map(|(id, label, port)| {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), *port);
            let found = TcpStream::connect_timeout(&addr, Duration::from_millis(220)).is_ok();
            CoreCandidate {
                id,
                label,
                url: format!("http://127.0.0.1:{port}"),
                found,
            }
        })
        .collect()
}
