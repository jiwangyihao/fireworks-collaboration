use fireworks_collaboration_lib::core::git::utils::{parse_depth, resolve_push_credentials};
use serde_json::json;

#[test]
fn test_parse_depth() {
    assert_eq!(parse_depth(None), None);
    assert_eq!(parse_depth(Some(json!(null))), None);
    assert_eq!(parse_depth(Some(json!(1))), Some(1));
    assert_eq!(parse_depth(Some(json!(100))), Some(100));
    assert_eq!(parse_depth(Some(json!("invalid"))), None);
}

#[test]
fn test_resolve_push_credentials_use_stored_success() {
    let (u, p) = resolve_push_credentials(
        true,
        None,
        None,
        Some("stored_user".into()),
        Some("stored_pass".into()),
    );
    assert_eq!(u.as_deref(), Some("stored_user"));
    assert_eq!(p.as_deref(), Some("stored_pass"));
}

#[test]
fn test_resolve_push_credentials_explicit_override() {
    // User provides credentials, ignoring stored (or use_stored=true but input takes precedence? logic says input is None check)
    // The logic: if use_stored && input_username.is_none() && input_password.is_none() ...

    let (u, p) = resolve_push_credentials(
        true,
        Some("input_user".into()),
        Some("input_pass".into()),
        Some("stored_user".into()),
        Some("stored_pass".into()),
    );
    // Logic should return input because inputs are NOT none
    assert_eq!(u.as_deref(), Some("input_user"));
    assert_eq!(p.as_deref(), Some("input_pass"));
}

#[test]
fn test_resolve_push_credentials_no_stored() {
    let (u, p) = resolve_push_credentials(true, None, None, None, None);
    // Should fallback to inputs (which are None)
    assert_eq!(u, None);
    assert_eq!(p, None);
}

#[test]
fn test_resolve_push_credentials_disabled_stored() {
    let (u, p) = resolve_push_credentials(
        false,
        None,
        None,
        Some("stored_user".into()),
        Some("stored_pass".into()),
    );
    // Should ignore stored
    assert_eq!(u, None);
    assert_eq!(p, None);
}
