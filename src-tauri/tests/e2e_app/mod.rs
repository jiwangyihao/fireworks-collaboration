//! App-Level E2E Tests
//!
//! Tests full user journeys using `fireworks_collaboration::app::commands` to drive logic,
//! validating the interaction between frontend-facing commands and the core logic (git2).

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod basic_flow;
mod pipeline;
