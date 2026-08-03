//! The `--update` flag: reinstall this binary at its latest published version.
//!
//! Delegates to `cargo install`, which already resolves "latest" against
//! crates.io. Doing the request here would mean adding a network stack, a TLS
//! stack and a registry client to a tool that otherwise only reads files, in
//! order to reimplement what the toolchain that installed it does correctly.

use std::process::Command;

/// The crate published to crates.io.
///
/// Deliberately different from the binary name, which is `lucyd`: that one is
/// already taken on crates.io by the library facade.
const PACKAGE: &str = "lucyd-cli";

/// Reinstalls this binary at the latest version published to crates.io.
///
/// `--force` is what makes this an update rather than a no-op: without it
/// `cargo install` declines as soon as any version is already installed.
pub fn run() -> Result<(), String> {
    println!("Updating {PACKAGE} from crates.io...");

    let status = Command::new("cargo")
        .args(["install", PACKAGE, "--force"])
        .status()
        .map_err(missing_cargo)?;

    if status.success() {
        return Ok(());
    }
    Err(format!(
        "`cargo install {PACKAGE} --force` exited with status {status}"
    ))
}

/// Explains a `cargo` that could not be launched.
///
/// Nothing else in this binary needs a toolchain at runtime, so a missing
/// `cargo` is surprising enough to deserve saying why it is needed.
fn missing_cargo(error: std::io::Error) -> String {
    format!(
        "failed to run `cargo`: {error}\n  \
         `--update` reinstalls through the Rust toolchain, which has to be on PATH."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_crate_is_not_the_binary_name() {
        assert_eq!(
            PACKAGE, "lucyd-cli",
            "installing `lucyd` would fetch the library facade, not this tool"
        );
    }

    #[test]
    fn a_launch_failure_says_a_toolchain_is_needed() {
        let error = missing_cargo(std::io::Error::from(std::io::ErrorKind::NotFound));

        assert!(error.contains("PATH"), "{error}");
    }
}
