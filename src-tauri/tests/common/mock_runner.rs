use fireworks_collaboration_lib::core::git::errors::GitError;
use fireworks_collaboration_lib::core::git::runner::GitRunner;
use std::collections::VecDeque;
use std::path::Path;
use std::process::Output;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockGitRunner {
    pub expectations: Arc<Mutex<VecDeque<MockExpectation>>>,
}

pub struct MockExpectation {
    pub args: Option<Vec<String>>,
    pub output: Result<Output, GitError>,
}

impl MockGitRunner {
    pub fn new() -> Self {
        Self {
            expectations: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn expect(&self, args: Option<Vec<&str>>, output: Result<Output, GitError>) {
        self.expectations
            .lock()
            .unwrap()
            .push_back(MockExpectation {
                args: args.map(|a| a.iter().map(|s| s.to_string()).collect()),
                output,
            });
    }
}

impl GitRunner for MockGitRunner {
    fn run(&self, args: &[&str], path: &Path) -> Result<Output, GitError> {
        let mut expectations = self.expectations.lock().unwrap();
        if let Some(expectation) = expectations.pop_front() {
            if let Some(expected_args) = expectation.args {
                let current_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
                if current_args != expected_args {
                    panic!(
                        "Unexpected arguments. Expected: {:?}, Got: {:?}",
                        expected_args, current_args
                    );
                }
            }
            expectation.output
        } else {
            panic!(
                "No more expectations for GitRunner. Call was: {:?} at {:?}",
                args, path
            );
        }
    }
}
