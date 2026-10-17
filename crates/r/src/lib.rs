//! The R script runner: the script and data model, the [`RScriptRunner`] trait, and the runners.
//! Native builds run a script with the `Rscript` program in a fresh job folder (`native.rs`). The
//! browser runner refuses every script until webR is wired up (`web.rs`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod job;
pub mod manifest;
pub mod notices;
pub mod stage;
pub mod wire;

#[cfg(not(target_arch = "wasm32"))]
mod discover;
#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod discover_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod harness_tests;
#[cfg(test)]
mod job_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod stage_tests;
#[cfg(test)]
mod tests;

#[cfg(not(target_arch = "wasm32"))]
pub use discover::{choose, find, ready, version_of};
#[cfg(not(target_arch = "wasm32"))]
pub use native::RscriptRunner;

/// The R harness: the bootstrap script that runs each job, embedded as `BOOTSTRAP_R`.
pub mod harness {
    pub const BOOTSTRAP_R: &str = include_str!("bootstrap.R");
}

pub use notices::RNotice;
pub use wire::{
    DEFAULT_TIMEOUT_S, MAX_FILE_BYTES, MAX_FILES, MAX_MANIFEST_BYTES, MAX_MESSAGES, MAX_SCRIPT_BYTES, RExecutionInput, RExecutionOutput,
    RFile, RLayerInput, RLayerKind, clamp_timeout, clean_lang,
};

use serde::{Deserialize, Serialize};

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
    #[error("{}", crate::notices::NOT_FOUND)]
    NotFound,
    #[error("{}", .0.english())]
    Notice(RNotice),
    #[error("not supported here: {0}")]
    Unsupported(String),
    #[error("R script failed: {0}")]
    Failed(String),
}

impl From<String> for RRunnerError {
    fn from(message: String) -> Self {
        RRunnerError::Failed(message)
    }
}

impl From<&str> for RRunnerError {
    fn from(message: &str) -> Self {
        RRunnerError::Failed(message.to_string())
    }
}

/// Runs a script. `progress` may be called any number of times while the script runs.
pub trait RScriptRunner: Send + Sync {
    fn run(&self, input: &RExecutionInput, progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError>;
}

/// The native runner: the Rscript program. There is no runner in the browser build yet.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_runner() -> Box<dyn RScriptRunner> {
    Box::new(RscriptRunner)
}
