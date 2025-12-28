use crate::common::mock_runner::MockGitRunner;
use fireworks_collaboration_lib::core::submodule::operations::SubmoduleManager;
use fireworks_collaboration_lib::core::submodule::SubmoduleConfig;
use std::path::Path;
use std::process::ExitStatus;
use std::process::Output;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

fn create_success_output() -> Output {
    // Create a successful ExitStatus
    #[cfg(unix)]
    let status = ExitStatus::from_raw(0);
    #[cfg(windows)]
    let status = ExitStatus::from_raw(0);

    Output {
        status,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

#[test]
fn test_submodule_init_calls_git_init() {
    let runner = MockGitRunner::new();
    let runner_clone = runner.clone();

    // Expect: git submodule init
    runner.expect(Some(vec!["submodule", "init"]), Ok(create_success_output()));

    let config = SubmoduleConfig::default();
    let manager = SubmoduleManager::new(config, Box::new(runner_clone));

    let path = Path::new("/tmp/test_repo");
    let result = manager.init_all(path);

    assert!(result.is_ok());

    // Verify all expectations were met happens on drop or explicitly
    // MockGitRunner panic on drop if expectations remain is not implemented,
    // but the `expect` method pushes to a queue and `run` pops.
    // Ideally we should verify queue is empty.
    assert!(runner.expectations.lock().unwrap().is_empty());
}

#[test]
fn test_submodule_update_calls_git_update() {
    let runner = MockGitRunner::new();
    let runner_clone = runner.clone();

    // Expect: git submodule update --init --recursive
    runner.expect(
        Some(vec!["submodule", "update", "--init", "--recursive"]),
        Ok(create_success_output()),
    );

    let config = SubmoduleConfig::default();
    let manager = SubmoduleManager::new(config, Box::new(runner_clone));

    let path = Path::new("/tmp/test_repo");
    let result = manager.update_all(path, 0); // start at depth 0

    assert!(result.is_ok());
    assert!(runner.expectations.lock().unwrap().is_empty());
}

#[test]
fn test_submodule_sync_calls_git_sync() {
    let runner = MockGitRunner::new();
    let runner_clone = runner.clone();

    // Expect: git submodule sync --recursive
    runner.expect(
        Some(vec!["submodule", "sync", "--recursive"]),
        Ok(create_success_output()),
    );

    let config = SubmoduleConfig::default();
    let manager = SubmoduleManager::new(config, Box::new(runner_clone));

    let path = Path::new("/tmp/test_repo");
    let result = manager.sync_all(path);

    assert!(result.is_ok());
    assert!(runner.expectations.lock().unwrap().is_empty());
}
