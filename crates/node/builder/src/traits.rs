//! Trait that defines the components required to build a kora node.

// Re-export common types for convenience
pub use commonware_consensus::simplex::elector::Random;
use commonware_consensus::{
    CertifiableAutomaton, Relay, Reporter,
    simplex::{
        self,
        elector::Config as ElectorConfig,
        types::{Activity, Context},
    },
};
use commonware_cryptography::{Digest, certificate::Scheme};
use commonware_p2p::Blocker;
use commonware_parallel::{Sequential, Strategy};

/// Node components.
pub trait NodeComponents: ConsensusProvider {}

/// Consensus provider.
///
/// Provides the simplex configuration for the consensus engine.
///
/// # Default Types
///
/// - `Strategy`: Defaults to [`Sequential`] for simple sequential execution
///
/// For `Elector`, use [`Random`] (re-exported) when using a BLS threshold VRF scheme.
pub trait ConsensusProvider {
    /// The signing scheme used by consensus.
    type Scheme: Scheme;

    /// The leader elector configuration.
    ///
    /// Use [`Random`] for VRF-based unpredictable leader selection
    /// when using a BLS threshold scheme.
    type Elector: ElectorConfig<Self::Scheme>;

    /// The network blocker.
    type Blocker: Blocker<PublicKey = <Self::Scheme as Scheme>::PublicKey>;

    /// The digest type for payloads.
    type Digest: Digest;

    /// The certifiable automaton (application interface).
    type Automaton: CertifiableAutomaton<
            Context = Context<Self::Digest, <Self::Scheme as Scheme>::PublicKey>,
            Digest = Self::Digest,
        >;

    /// The relay for broadcasting payloads.
    type Relay: Relay<Digest = Self::Digest>;

    /// The activity reporter.
    type Reporter: Reporter<Activity = Activity<Self::Scheme, Self::Digest>>;

    /// The parallel execution strategy.
    ///
    /// Defaults to [`Sequential`] for simple sequential execution.
    type Strategy: Strategy = Sequential;

    /// Returns the [`simplex::Config`] used by the node.
    ///
    /// The config is used to construct the [`simplex::Engine`]
    /// which is responsible for driving consensus.
    #[allow(clippy::type_complexity)]
    fn simplex_config(
        &self,
    ) -> simplex::Config<
        Self::Scheme,
        Self::Elector,
        Self::Blocker,
        Self::Digest,
        Self::Automaton,
        Self::Relay,
        Self::Reporter,
        Self::Strategy,
    >;
}
