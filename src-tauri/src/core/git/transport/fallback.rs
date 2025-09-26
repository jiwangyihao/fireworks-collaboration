//! Fallback decision scaffolding for adaptive TLS transport (P3.0).
//!
//! Goal (P3.0): extract a pure, easily testable state machine describing the
//! Fake -> Real -> Default fallback chain WITHOUT altering existing runtime
//! behavior yet. Subsequent stages (P3.1+) will plug rollout sampling,
//! metrics, fingerprinting and runtime auto-disable into this layer.
//!
//! This module purposely keeps implementation minimal and deterministic:
//! - No global state / I/O
//! - No timing / hashing / randomness
//! - Pure enum & transition functions
//!
//! Existing transport code today performs (conceptually):
//!   1. Try Fake SNI (if enabled & whitelist hit & no proxy)
//!   2. On TLS handshake failure -> retry with Real SNI
//!   3. If Real also fails -> propagate error (libgit2 default path happens
//!      when URL not rewritten in the first place)
//!
//! After we integrate this module the transport code will *query* it for the
//! next stage instead of embedding ad-hoc branching.
//!
//! NOTE: For P3.0 integration we will not emit events or metrics from here –
//! just expose structured reasons for later instrumentation.

/// High-level stage of the adaptive chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackStage {
    /// No adaptive attempt yet (initial state).
    None,
    /// Using Fake SNI attempt.
    Fake,
    /// Using Real SNI attempt.
    Real,
    /// Reverted to baseline (libgit2 default / no custom TLS).
    Default,
}

impl FallbackStage {
    pub fn is_terminal(self) -> bool {
        matches!(self, FallbackStage::Default)
    }
}

/// Categorised technical reason that caused a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// First stage selected by policy (rollout / whitelist) to try Fake.
    EnterFake,
    /// TLS handshake (or pre-TLS TCP) failure while using Fake SNI.
    FakeHandshakeError,
    /// Policy decided to skip Fake (sampling miss / disabled / proxy present).
    SkipFakePolicy,
    /// Real SNI attempt also failed, no further custom attempts allowed.
    RealFailed,
}

/// A single transition output produced by the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackTransition {
    pub from: FallbackStage,
    pub to: FallbackStage,
    pub reason: FallbackReason,
}

/// Decision context flags passed by caller (pure data – computed externally).
#[derive(Debug, Clone, Default)]
pub struct DecisionCtx {
    /// Global `fakeSniEnabled` and host whitelist + scheme + no proxy etc.
    pub policy_allows_fake: bool,
    /// Whether an earlier runtime circuit-breaker temporarily disabled Fake.
    pub runtime_fake_disabled: bool,
}

/// State container; progresses via `advance_on_error` or `initial`.
#[derive(Debug, Clone)]
pub struct FallbackDecision {
    stage: FallbackStage,
    /// Transitions taken so far (deterministic order). Useful for tests &
    /// later metrics aggregation.
    history: Vec<FallbackTransition>,
}

impl FallbackDecision {
    /// Create initial decision based on policy flags.
    pub fn initial(ctx: &DecisionCtx) -> Self {
        if ctx.policy_allows_fake && !ctx.runtime_fake_disabled {
            let mut d = Self {
                stage: FallbackStage::Fake,
                history: Vec::new(),
            };
            d.history.push(FallbackTransition {
                from: FallbackStage::None,
                to: FallbackStage::Fake,
                reason: FallbackReason::EnterFake,
            });
            d
        } else {
            let mut d = Self {
                stage: FallbackStage::Default,
                history: Vec::new(),
            };
            d.history.push(FallbackTransition {
                from: FallbackStage::None,
                to: FallbackStage::Default,
                reason: FallbackReason::SkipFakePolicy,
            });
            d
        }
    }

    pub fn stage(&self) -> FallbackStage {
        self.stage
    }
    pub fn history(&self) -> &[FallbackTransition] {
        &self.history
    }

    /// Called when an attempt at the current stage fails (e.g. TLS handshake error).
    /// Returns Some(transition) if a new stage is entered; None if terminal.
    pub fn advance_on_error(&mut self) -> Option<FallbackTransition> {
        match self.stage {
            FallbackStage::Fake => {
                // Move to Real
                let tr = FallbackTransition {
                    from: FallbackStage::Fake,
                    to: FallbackStage::Real,
                    reason: FallbackReason::FakeHandshakeError,
                };
                self.stage = FallbackStage::Real;
                self.history.push(tr.clone());
                Some(tr)
            }
            FallbackStage::Real => {
                let tr = FallbackTransition {
                    from: FallbackStage::Real,
                    to: FallbackStage::Default,
                    reason: FallbackReason::RealFailed,
                };
                self.stage = FallbackStage::Default;
                self.history.push(tr.clone());
                Some(tr)
            }
            FallbackStage::Default | FallbackStage::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_fake_path() {
        let ctx = DecisionCtx {
            policy_allows_fake: true,
            runtime_fake_disabled: false,
        };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Fake);
        assert_eq!(d.history().len(), 1);
        assert_eq!(d.history()[0].reason, FallbackReason::EnterFake);
    }

    #[test]
    fn initial_skip_path() {
        let ctx = DecisionCtx {
            policy_allows_fake: false,
            runtime_fake_disabled: false,
        };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        assert_eq!(d.history()[0].reason, FallbackReason::SkipFakePolicy);
    }

    #[test]
    fn advance_chain() {
        let ctx = DecisionCtx {
            policy_allows_fake: true,
            runtime_fake_disabled: false,
        };
        let mut d = FallbackDecision::initial(&ctx);
        let tr1 = d.advance_on_error().expect("fake->real");
        assert_eq!(tr1.to, FallbackStage::Real);
        let tr2 = d.advance_on_error().expect("real->default");
        assert_eq!(tr2.to, FallbackStage::Default);
        assert!(d.advance_on_error().is_none(), "default is terminal");
        assert_eq!(d.history().len(), 3);
    }

    #[test]
    fn default_stage_is_idempotent() {
        let ctx = DecisionCtx {
            policy_allows_fake: false,
            runtime_fake_disabled: false,
        }; // initial -> Default
        let mut d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        assert!(d.advance_on_error().is_none());
        assert_eq!(
            d.history().len(),
            1,
            "history should not grow after terminal advance attempts"
        );
    }
}
