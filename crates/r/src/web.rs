//! The browser runner. Scripts run through webR, which is not wired up yet.

use crate::{ExecutionUpdate, RExecutionInput, RExecutionOutput, RRunnerError, RScriptRunner};

/// Refuses every script until webR is wired up.
#[derive(Debug, Default, Clone, Copy)]
pub struct WebRRunner;

impl RScriptRunner for WebRRunner {
    fn run(&self, _input: &RExecutionInput, _progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError> {
        Err(RRunnerError::Unsupported("R scripts run in the browser through webR, which is not wired up yet".into()))
    }
}
