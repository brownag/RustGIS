//! The native runner. A stub: it finds `Rscript` but does not execute the script yet.

use std::path::PathBuf;
use std::process::Command;

use crate::{ExecutionUpdate, RExecutionInput, RExecutionOutput, RRunnerError, RScriptRunner};

/// Runs scripts with the `Rscript` program.
#[derive(Debug, Default, Clone, Copy)]
pub struct RscriptRunner;

impl RscriptRunner {
    /// The `Rscript` program: `RUSTGIS_RSCRIPT` if set, else `Rscript` on the PATH.
    pub fn locate(&self) -> Result<PathBuf, RRunnerError> {
        if let Some(path) = std::env::var_os("RUSTGIS_RSCRIPT").filter(|p| !p.is_empty()) {
            return Ok(PathBuf::from(path));
        }
        match Command::new("Rscript").arg("--version").output() {
            Ok(out) if out.status.success() => Ok(PathBuf::from("Rscript")),
            Ok(out) => Err(RRunnerError::Failed(format!("Rscript --version exited with {}", out.status))),
            Err(_) => Err(RRunnerError::NotFound),
        }
    }
}

impl RScriptRunner for RscriptRunner {
    fn run(&self, _input: &RExecutionInput, progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError> {
        progress(ExecutionUpdate::Progress(1.0));
        Ok(RExecutionOutput { tables: Vec::new(), stdout: String::new(), metadata: serde_json::json!({ "stub": true }) })
    }
}
