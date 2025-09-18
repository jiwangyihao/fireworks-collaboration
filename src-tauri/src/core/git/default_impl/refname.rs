//! Common reference name validation utilities for branch / tag / remote names.
//! Centralizes rules so future commands (tag / remote) reuse identical semantics.
//! All violations map to Protocol category (caller can fix input).

use super::super::errors::{GitError, ErrorCategory};

/// Validate a generic ref-like name (without the refs/heads/ prefix) according to
/// a conservative subset of git reference name rules we enforce in this product.
/// We purposely do not attempt to re‑implement the full spec but cover common
/// invalid patterns to provide clear user feedback while minimizing false negatives.
pub fn validate_ref_name(raw: &str) -> Result<(), GitError> {
	let name = raw.trim();
	if name.is_empty() { return err("name is empty"); }
	if name.contains(' ') { return err("name contains space"); }
	if name.starts_with('/') { return err("name starts with '/'"); }
	if name.contains("//") { return err("name contains '//'"); }
	if name.starts_with('-') { return err("name starts with '-'"); }
	if name.ends_with('/') || name.ends_with('.') { return err("name ends with invalid char"); }
	if name.ends_with(".lock") { return err("name ends with .lock"); }
	if name.contains("..") { return err("name contains '..'"); }
	if name.contains('\\') { return err("name contains backslash"); }
	// Illegal single characters we disallow (common problematic set)
	const ILLEGAL: &[char] = &[':', '?', '*', '[', '~', '^'];
	if name.chars().any(|c| ILLEGAL.contains(&c)) { return err("name contains illegal char"); }
	if name.contains("@{") { return err("name contains '@{'"); }
	if name.chars().any(|c| c.is_control()) { return err("name has control char"); }
	Ok(())
}

#[inline]
fn err(msg: &str) -> Result<(), GitError> { Err(GitError::new(ErrorCategory::Protocol, msg)) }

/// Branch specific wrapper (currently identical to generic behavior).
pub fn validate_branch_name(name: &str) -> Result<(), GitError> { validate_ref_name(name) }

/// Tag specific wrapper – identical now; future: may diverge (e.g. allow leading 'v').
pub fn validate_tag_name(name: &str) -> Result<(), GitError> { validate_ref_name(name) }

/// Remote name wrapper – may impose stricter rules later (e.g. disallow slash entirely).
pub fn validate_remote_name(name: &str) -> Result<(), GitError> { validate_ref_name(name) }

