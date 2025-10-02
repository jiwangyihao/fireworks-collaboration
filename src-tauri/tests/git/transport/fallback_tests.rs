use fireworks_collaboration_lib::core::git::transport::{
    DecisionCtx, FallbackDecision, FallbackReason, FallbackStage,
};

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
