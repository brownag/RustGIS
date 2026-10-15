//! The R script runner, scaffold only: the script and data model, the [`RScriptRunner`] trait,
//! and stub runners. Native runs no R code yet; the browser runner reports `Unsupported`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(test)]
mod tests;

#[cfg(not(target_arch = "wasm32"))]
pub use native::RscriptRunner;
#[cfg(target_arch = "wasm32")]
pub use web::WebRRunner;

use serde::{Deserialize, Serialize};

/// A layer handed to a script: its name, its format and its bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RLayerInput {
    pub name: String,
    pub format: String,
    pub data: Vec<u8>,
}

/// What a script runs on: the code, its input layers and its arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RExecutionInput {
    pub script: String,
    pub layers: Vec<RLayerInput>,
    pub args: serde_json::Value,
}

/// What a script produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RExecutionOutput {
    pub tables: Vec<serde_json::Value>,
    pub stdout: String,
    pub metadata: serde_json::Value,
}

/// The severity of a log line from a running script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// A progress report or log line from a running script.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionUpdate {
    Progress(f32),
    Log { level: LogLevel, message: String },
}

/// Why a script could not run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RRunnerError {
    #[error("Rscript was not found")]
    NotFound,
    #[error("not supported here: {0}")]
    Unsupported(String),
    #[error("R script failed: {0}")]
    Failed(String),
}

/// Runs a script. `progress` may be called any number of times while the script runs.
pub trait RScriptRunner: Send + Sync {
    fn run(&self, input: &RExecutionInput, progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError>;
}

/// The runner for this build target.
pub fn default_runner() -> Box<dyn RScriptRunner> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Box::new(RscriptRunner)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Box::new(WebRRunner)
    }
}
