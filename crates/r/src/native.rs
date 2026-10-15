//! The native runner. A stub: it finds `Rscript` but does not execute the script yet.

use std::path::PathBuf;

use crate::{ExecutionUpdate, RExecutionInput, RExecutionOutput, RRunnerError, RScriptRunner};

/// Runs scripts with the `Rscript` program.
#[derive(Debug, Default, Clone, Copy)]
pub struct RscriptRunner;

impl RscriptRunner {
    /// The `Rscript` program that answers `--version` (see `discover::find`).
    pub fn locate(&self) -> Result<PathBuf, RRunnerError> {
        crate::discover::find().map(|(exe, _)| exe).ok_or(RRunnerError::NotFound)
    }
}

impl RScriptRunner for RscriptRunner {
    fn run(&self, _input: &RExecutionInput, progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError> {
        progress(ExecutionUpdate::Progress(1.0));
        Ok(RExecutionOutput {
            tables: Vec::new(),
            stdout: String::new(),
            metadata: serde_json::json!({ "stub": true }),
            ..Default::default()
        })
    }
}
