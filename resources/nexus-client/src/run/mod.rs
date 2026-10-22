//! Runs the command line [Commands](nexus:markdown.section (README.md))
//! describes, finding `nexus` on the `PATH`.

use std::path::Path;
use std::process::Command;

/// What a command printed, and whether it succeeded.
pub struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// The output, or what the command printed on standard error when it
    /// failed for a reason other than finding nothing.
    pub fn into_result(self) -> Result<Self, String> {
        if !self.success && !self.stderr.trim().is_empty() {
            return Err(self.stderr.trim().to_owned());
        }
        Ok(self)
    }
}

/// Runs `nexus` with [`arguments`](nexus:rust.argument) on the repository at
/// [`root`](nexus:rust.argument).
pub fn nexus(root: &Path, arguments: &[&str]) -> Result<Output, String> {
    let output = Command::new("nexus")
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|error| format!("running nexus: {error}"))?;
    Ok(Output {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
