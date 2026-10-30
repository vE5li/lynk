//! Runs the command line [Commands](lynk:markdown.section (README.md))
//! describes, finding `lynk` on the `PATH`.

use std::fmt;
use std::path::Path;
use std::process::Command;

/// The program run, looked up on the `PATH`.
const PROGRAM: &str = "lynk";

/// Why a command gave nothing to read.
#[derive(Debug)]
pub enum Error {
    /// `lynk` could not be started, most likely missing from the `PATH`.
    Start(std::io::Error),
    /// It ran and failed for a reason other than finding nothing.
    Failed { stderr: String },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Start(error) => write!(formatter, "running lynk: {error}"),
            Self::Failed { stderr } => formatter.write_str(stderr),
        }
    }
}

/// The workspace's commands report errors as text.
impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}

/// What a command printed, and whether it succeeded.
pub struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// The output, or what the command printed on standard error when it
    /// failed for a reason other than finding nothing.
    pub fn into_result(self) -> Result<Self, Error> {
        if !self.success && !self.stderr.trim().is_empty() {
            return Err(Error::Failed {
                stderr: self.stderr.trim().to_owned(),
            });
        }
        Ok(self)
    }
}

/// Runs `lynk` with [`arguments`](lynk:rust.argument) on the repository at
/// [`root`](lynk:rust.argument).
pub fn lynk(root: &Path, arguments: &[&str]) -> Result<Output, Error> {
    let output = Command::new(PROGRAM)
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(Error::Start)?;
    Ok(Output {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
