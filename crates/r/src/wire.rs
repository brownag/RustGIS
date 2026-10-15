//! The wire types for R scripts: what a script is given and what it returns, and the limits
//! both sides share.

use serde::{Deserialize, Serialize};

/// The largest script text a run takes (1 MB).
pub const MAX_SCRIPT_BYTES: usize = 1 << 20;

/// The largest file sent to or read from a run: 512 MB natively.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_FILE_BYTES: usize = 512 << 20;

/// The largest file sent to or read from a run: 256 MB in the browser.
#[cfg(target_arch = "wasm32")]
pub const MAX_FILE_BYTES: usize = 256 << 20;

/// The most files a run takes or returns.
pub const MAX_FILES: usize = 256;

/// The largest manifest a run may write (8 MB).
pub const MAX_MANIFEST_BYTES: usize = 8 << 20;

/// The most progress and log messages kept from a run.
pub const MAX_MESSAGES: usize = 1000;

/// The default time limit of a run, in seconds.
pub const DEFAULT_TIMEOUT_S: u64 = 120;

/// The kind of data a layer holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RLayerKind {
    /// Features with geometry.
    #[default]
    Vector,
    /// Attribute rows with no geometry.
    Table,
    /// A raster grid.
    Raster,
}

/// A layer handed to a script: its name, its format and its bytes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RLayerInput {
    pub name: String,
    /// `fgb`, `csv` or `tif`.
    pub format: String,
    pub data: Vec<u8>,
    #[serde(default)]
    pub kind: RLayerKind,
    /// The positions (0-based) within the rows staged in `data` that the script gets.
    #[serde(default)]
    pub selection: Vec<usize>,
}

/// What a script runs on: the code, its input layers and its arguments.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RExecutionInput {
    pub script: String,
    pub layers: Vec<RLayerInput>,
    pub args: serde_json::Value,
    /// The time limit in seconds. `None` takes the default (see [`clamp_timeout`]).
    #[serde(default)]
    pub timeout_s: Option<u64>,
    /// The language for messages from the script, such as `fr`. See [`clean_lang`].
    #[serde(default)]
    pub lang: String,
}

/// A file a script wrote, by its path under `out/`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RFile {
    pub path: String,
    pub data: Vec<u8>,
}

/// What a script produced.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RExecutionOutput {
    pub tables: Vec<serde_json::Value>,
    pub stdout: String,
    pub metadata: serde_json::Value,
    /// The files the script wrote to `out/`.
    #[serde(default)]
    pub files: Vec<RFile>,
}

/// The time limit to use: the requested seconds kept between 1 and 86 400 (one day), or the
/// default of 120 seconds when none is asked for.
pub fn clamp_timeout(requested: Option<u64>) -> u64 {
    requested.map_or(DEFAULT_TIMEOUT_S, |s| s.clamp(1, 86_400))
}

/// A language tag safe to pass on: ASCII letters, digits, `_` and `-`, at most 16 characters.
/// Anything else becomes `en`.
pub fn clean_lang(lang: &str) -> String {
    let ok = !lang.is_empty() && lang.len() <= 16 && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok { lang.to_string() } else { "en".to_string() }
}
