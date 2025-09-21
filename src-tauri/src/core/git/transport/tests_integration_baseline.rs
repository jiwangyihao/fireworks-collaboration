#[cfg(test)]
mod tests {
    use crate::core::config::model::AppConfig;
    use crate::core::git::transport::{DecisionCtx, FallbackDecision, FallbackStage};

    #[test]
    fn initial_stage_default_when_disabled() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = false;
        let ctx = DecisionCtx { policy_allows_fake: cfg.http.fake_sni_enabled, runtime_fake_disabled: false };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
    }
}
