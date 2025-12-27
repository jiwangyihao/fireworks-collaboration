use serde_json::Value;

/// Helper to parse optional depth parameter from JSON value.
pub fn parse_depth(depth: Option<Value>) -> Option<u32> {
    depth.and_then(|v| v.as_u64().map(|x| x as u32))
}

/// pure function to resolve final username and password logic
/// returns (final_username, final_password)
pub fn resolve_push_credentials(
    use_stored: bool,
    input_username: Option<String>,
    input_password: Option<String>,
    stored_username: Option<String>,
    stored_password: Option<String>,
) -> (Option<String>, Option<String>) {
    if use_stored && input_username.is_none() && input_password.is_none() {
        if let (Some(u), Some(p)) = (stored_username, stored_password) {
            return (Some(u), Some(p));
        }
        // If no stored credentials, use provided (which are None)
    }
    (input_username, input_password)
}
