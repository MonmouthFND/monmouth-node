//! Network transport bundle.

use commonware_cryptography::PublicKey;
use commonware_p2p::authenticated::discovery;
use commonware_runtime::{Clock, Handle};

use crate::channels::{MarshalChannels, SimplexChannels};

/// Complete network transport bundle.
///
/// Contains everything needed to wire up consensus and application layers:
/// - The oracle for peer management and blocking
/// - All 5 channel pairs grouped by consumer
/// - The network handle to keep it alive
///
/// # Channel Groups
///
/// Channels are grouped by their consumer:
/// - [`SimplexChannels`]: For consensus engine (votes, certs, resolver)
/// - [`MarshalChannels`]: For block dissemination (blocks, backfill)
#[allow(missing_debug_implementations)]
pub struct NetworkTransport<P: PublicKey, E: Clock> {
    /// Oracle for peer management and Byzantine blocking.
    ///
    /// Implements both [`Manager`](commonware_p2p::Manager) and
    /// [`Blocker`](commonware_p2p::Blocker) traits.
    pub oracle: discovery::Oracle<P>,

    /// Network handle to keep the network task alive.
    ///
    /// Drop this and the network shuts down.
    pub handle: Handle<()>,

    /// Channels for consensus engine (simplex).
    pub simplex: SimplexChannels<P, E>,

    /// Channels for block dissemination and backfill (marshal).
    pub marshal: MarshalChannels<P, E>,
}
