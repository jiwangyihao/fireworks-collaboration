use fireworks_collaboration_lib::core::git::utils::{parse_depth, resolve_push_credentials};
use serde_json::json;

#[test]
fn test_parse_depth() {
    // None
    assert_eq!(parse_depth(None), None);

    // Null
    assert_eq!(parse_depth(Some(json!(null))), None);

    // Integers
    assert_eq!(parse_depth(Some(json!(1))), Some(1));
    assert_eq!(parse_depth(Some(json!(50))), Some(50));

    // String (should be None currently, based on implementation)
    // Implementation: depth.and_then(|v| v.as_u64().map(|x| x as u32))
    assert_eq!(parse_depth(Some(json!("1"))), None);

    // Float (as_u64 fails for floats usually unless they are exact integers? json! macro makes them Number)
    // serde_json::Value::as_u64 returns Some only if it's an integer.
    assert_eq!(parse_depth(Some(json!(1.0))), None); // 1.0 might be treated as float by macro?
                                                     // Actually json!(1) is Number(1). json!(1.0) is Number(1.0).
                                                     // Let's verify strict behavior.
}

#[test]
fn test_resolve_push_credentials() {
    // 1. All None
    assert_eq!(
        resolve_push_credentials(false, None, None, None, None),
        (None, None)
    );

    // 2. Input provided (priority)
    assert_eq!(
        resolve_push_credentials(
            true,
            Some("input_u".into()),
            Some("input_p".into()),
            Some("store_u".into()),
            Some("store_p".into())
        ),
        (Some("input_u".into()), Some("input_p".into()))
    );

    // 3. Use stored (only if input is None)
    assert_eq!(
        resolve_push_credentials(
            true,
            None,
            None,
            Some("store_u".into()),
            Some("store_p".into())
        ),
        (Some("store_u".into()), Some("store_p".into()))
    );

    // 4. Use stored = false (even if stored avail)
    assert_eq!(
        resolve_push_credentials(
            false,
            None,
            None,
            Some("store_u".into()),
            Some("store_p".into())
        ),
        (None, None)
    );

    // 5. Partial input (username only) -> Should NOT use stored?
    // Implementation: if use_stored && input_username.is_none() && input_password.is_none()
    // So if ANY input is present, it returns inputs.
    assert_eq!(
        resolve_push_credentials(
            true,
            Some("user".into()),
            None,
            Some("store_u".into()),
            Some("store_p".into())
        ),
        (Some("user".into()), None)
    );
}
