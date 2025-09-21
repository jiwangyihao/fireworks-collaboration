#[cfg(test)]
mod fallback_decision_tests {
    use crate::core::git::transport::{DecisionCtx, FallbackDecision, FallbackStage, FallbackReason};

    #[test]
    fn skip_fake_policy_creates_default_stage() {
        let ctx = DecisionCtx { policy_allows_fake: false, runtime_fake_disabled: false };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        let h = d.history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].reason, FallbackReason::SkipFakePolicy);
    }

    #[test]
    fn full_chain_history_order() {
        let ctx = DecisionCtx { policy_allows_fake: true, runtime_fake_disabled: false };
        let mut d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Fake);
        d.advance_on_error().expect("fake->real");
        d.advance_on_error().expect("real->default");
        assert!(d.advance_on_error().is_none());
        let stages: Vec<_> = d.history().iter().map(|tr| tr.to).collect();
        assert_eq!(stages, vec![FallbackStage::Fake, FallbackStage::Real, FallbackStage::Default]);
    }

    #[test]
    fn runtime_fake_disabled_behaves_like_policy_skip() {
        let ctx = DecisionCtx { policy_allows_fake: true, runtime_fake_disabled: true };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        assert_eq!(d.history()[0].reason, FallbackReason::SkipFakePolicy);
    }
}
