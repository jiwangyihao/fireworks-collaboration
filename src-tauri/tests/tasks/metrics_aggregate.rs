//! Metrics aggregation unit tests
//!
//! Tests for WindowRange, WindowResolution, TimeProvider, and basic aggregation.

use std::time::Duration;

use fireworks_collaboration_lib::core::metrics::{
    ManualTimeProvider, TimeProvider, WindowRange, WindowResolution,
};

// ============ WindowRange Tests ============

#[test]
fn test_window_range_resolution_last_minute() {
    assert_eq!(
        WindowRange::LastMinute.resolution(),
        WindowResolution::Minute
    );
}

#[test]
fn test_window_range_resolution_last_five_minutes() {
    assert_eq!(
        WindowRange::LastFiveMinutes.resolution(),
        WindowResolution::Minute
    );
}

#[test]
fn test_window_range_resolution_last_hour() {
    assert_eq!(WindowRange::LastHour.resolution(), WindowResolution::Minute);
}

#[test]
fn test_window_range_resolution_last_day() {
    assert_eq!(WindowRange::LastDay.resolution(), WindowResolution::Hour);
}

#[test]
fn test_window_range_slots_last_minute() {
    assert_eq!(WindowRange::LastMinute.slots(), 1);
}

#[test]
fn test_window_range_slots_last_five_minutes() {
    assert_eq!(WindowRange::LastFiveMinutes.slots(), 5);
}

#[test]
fn test_window_range_slots_last_hour() {
    assert_eq!(WindowRange::LastHour.slots(), 60);
}

#[test]
fn test_window_range_slots_last_day() {
    assert_eq!(WindowRange::LastDay.slots(), 24);
}

// ============ ManualTimeProvider Tests ============

#[test]
fn test_manual_time_provider_new() {
    let provider = ManualTimeProvider::new();
    // Should start at some base time
    let t1 = provider.now();
    let t2 = provider.now();
    assert_eq!(t1, t2); // Time doesn't advance without explicit call
}

#[test]
fn test_manual_time_provider_advance() {
    let provider = ManualTimeProvider::new();
    let t1 = provider.now();

    provider.advance(Duration::from_secs(10));
    let t2 = provider.now();

    assert!(t2 > t1);
    assert_eq!(t2.duration_since(t1), Duration::from_secs(10));
}

#[test]
fn test_manual_time_provider_advance_multiple() {
    let provider = ManualTimeProvider::new();
    let t1 = provider.now();

    provider.advance(Duration::from_secs(5));
    provider.advance(Duration::from_secs(10));
    let t2 = provider.now();

    assert_eq!(t2.duration_since(t1), Duration::from_secs(15));
}

#[test]
fn test_manual_time_provider_reset() {
    let provider = ManualTimeProvider::new();
    let t1 = provider.now();

    provider.advance(Duration::from_secs(100));
    provider.reset();
    let t2 = provider.now();

    assert_eq!(t1, t2);
}

#[test]
fn test_manual_time_provider_set() {
    let provider = ManualTimeProvider::new();
    let base = provider.now();

    provider.set(Duration::from_secs(50));
    let t = provider.now();

    assert_eq!(t.duration_since(base), Duration::from_secs(50));
}

#[test]
fn test_manual_time_provider_default() {
    let provider = ManualTimeProvider::default();
    let t1 = provider.now();
    provider.advance(Duration::from_millis(100));
    let t2 = provider.now();
    assert_eq!(t2.duration_since(t1), Duration::from_millis(100));
}
