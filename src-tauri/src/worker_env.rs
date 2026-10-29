//! Minimal environments for third-party ML worker processes.
//!
//! Bottie's MOSS image and Nano speech workers run third-party ML code (torch, diffusers,
//! ONNX, a Swift binary). They must not inherit secrets from the shell that launched Bottie,
//! such as `ANTHROPIC_API_KEY`, `HF_TOKEN` or proxy credentials. Each spawn clears the
//! environment and re-adds only the variables below, when the parent has them.

use std::ffi::{OsStr, OsString};

use tokio::process::Command;

/// Variables a worker may inherit: search path, home, temp dirs, locale, the Linux loader
/// path and CUDA device selection that GPU runtimes need, and the Windows system root.
const INHERITED_VARIABLES: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "LD_LIBRARY_PATH",
    "CUDA_HOME",
    "CUDA_PATH",
    "CUDA_VISIBLE_DEVICES",
    "SYSTEMROOT",
    "USERPROFILE",
];

/// Keeps only allowlisted variables from `variables`, preserving their values.
fn allowlisted(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    variables
        .into_iter()
        .filter(|(name, _)| {
            INHERITED_VARIABLES
                .iter()
                .any(|allowed| OsStr::new(allowed) == name)
        })
        .collect()
}

/// Clears the inherited environment and re-adds only the allowlisted parent variables.
///
/// Call this before any `.env(...)` the worker needs, because it starts from empty.
pub(crate) fn apply_minimal_environment(command: &mut Command) -> &mut Command {
    command.env_clear().envs(allowlisted(std::env::vars_os()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_allowlisted_variables() {
        let parent = [
            ("PATH", "/usr/bin"),
            ("ANTHROPIC_API_KEY", "secret"),
            ("HF_TOKEN", "secret"),
            ("HTTPS_PROXY", "http://user:pass@proxy"),
            ("HOME", "/Users/test"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));

        let kept = allowlisted(parent);

        assert_eq!(
            kept,
            vec![
                (OsString::from("PATH"), OsString::from("/usr/bin")),
                (OsString::from("HOME"), OsString::from("/Users/test")),
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawned_child_does_not_see_unlisted_parent_variables() {
        // Cargo sets CARGO_MANIFEST_DIR in the test process, so it stands in for a secret.
        assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
        let mut command = Command::new("/usr/bin/env");
        apply_minimal_environment(&mut command).env("HF_HUB_OFFLINE", "1");

        let output = command.output().await.expect("env should run");
        let listing = String::from_utf8(output.stdout).expect("env output is UTF-8");

        assert!(!listing.contains("CARGO_MANIFEST_DIR="));
        assert!(listing.lines().any(|line| line.starts_with("PATH=")));
        assert!(listing.lines().any(|line| line == "HF_HUB_OFFLINE=1"));
    }
}
