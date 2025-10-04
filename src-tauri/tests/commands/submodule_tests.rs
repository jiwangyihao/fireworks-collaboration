use fireworks_collaboration_lib::app::commands::submodule::SubmoduleCommandResult;

#[test]
fn test_submodule_command_result_ok() {
    let result = SubmoduleCommandResult::ok("success");
    assert!(result.success);
    assert_eq!(result.message, "success");
    assert!(result.data.is_none());
}

#[test]
fn test_submodule_command_result_ok_with_data() {
    let data = serde_json::json!({"count": 5});
    let result = SubmoduleCommandResult::ok_with_data("success", data.clone());
    assert!(result.success);
    assert_eq!(result.message, "success");
    assert_eq!(result.data, Some(data));
}

#[test]
fn test_submodule_command_result_err() {
    let result = SubmoduleCommandResult::err("error occurred");
    assert!(!result.success);
    assert_eq!(result.message, "error occurred");
    assert!(result.data.is_none());
}

#[test]
fn test_submodule_command_result_serde() {
    let result = SubmoduleCommandResult::ok("test");
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: SubmoduleCommandResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result.success, deserialized.success);
    assert_eq!(result.message, deserialized.message);
}
