//! The job builder: turns a run into the files an R process reads. The files are pure bytes, so
//! the native runner and the browser runner both write them the same way.
//!
//! The job directory holds `script.R`, `bootstrap.R`, `params.json` and one `in/<index>.<ext>`
//! per layer. Layers are named by index only, never by their names, so a hostile name never
//! reaches a file path.

use crate::harness::BOOTSTRAP_R;
use crate::wire::{MAX_FILE_BYTES, MAX_FILES, MAX_SCRIPT_BYTES, RExecutionInput, RFile, RLayerKind, clamp_timeout, clean_lang};
use serde_json::json;

/// The files of a job, in order: `script.R`, `bootstrap.R`, `params.json`, then `in/<index>.<ext>`
/// for each layer. Errors are plain English messages.
pub fn build_job_files(input: &RExecutionInput) -> Result<Vec<RFile>, String> {
    if input.script.trim().is_empty() {
        return Err("the script is empty".into());
    }
    if input.script.len() > MAX_SCRIPT_BYTES {
        return Err(format!("the script is larger than the {MAX_SCRIPT_BYTES}-byte limit"));
    }
    if input.layers.len() > MAX_FILES {
        return Err(format!("a run takes at most {MAX_FILES} layers"));
    }

    let mut layer_files = Vec::with_capacity(input.layers.len());
    let mut inputs = Vec::with_capacity(input.layers.len());
    for (index, layer) in input.layers.iter().enumerate() {
        check_layer_size(&layer.name, layer.data.len(), MAX_FILE_BYTES)?;
        let ext = ext_for(layer.kind, &layer.format)?;
        let selection = selection_rows(&layer.selection)?;
        let file = format!("in/{index}.{ext}");
        inputs.push(json!({
            "index": index,
            "name": layer.name,
            "kind": kind_name(layer.kind),
            "format": layer.format,
            "file": file,
            "selection": selection,
        }));
        layer_files.push(RFile { path: file, data: layer.data.clone() });
    }

    let args = if input.args.is_object() { input.args.clone() } else { json!({}) };
    let params = json!({
        "version": 1,
        "lang": clean_lang(&input.lang),
        "timeout_s": clamp_timeout(input.timeout_s),
        "args": args,
        "inputs": inputs,
    });
    let params_bytes = serde_json::to_vec(&params).map_err(|e| e.to_string())?;

    let mut files = vec![
        RFile { path: "script.R".into(), data: input.script.as_bytes().to_vec() },
        RFile { path: "bootstrap.R".into(), data: BOOTSTRAP_R.as_bytes().to_vec() },
        RFile { path: "params.json".into(), data: params_bytes },
    ];
    files.extend(layer_files);
    Ok(files)
}

/// True when every path is one a job builds: the three job files, or `in/<digits>.<ext>` with
/// a layer extension. Anything else (`..`, absolute, backslash, `out/`) is refused.
pub fn job_paths_ok(files: &[RFile]) -> bool {
    files.iter().all(|f| known_path(&f.path))
}

fn known_path(path: &str) -> bool {
    match path {
        "script.R" | "bootstrap.R" | "params.json" => true,
        _ => path.strip_prefix("in/").and_then(|rest| rest.split_once('.')).is_some_and(|(index, ext)| {
            !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()) && matches!(ext, "fgb" | "csv" | "tif")
        }),
    }
}

/// The name a layer kind has in `params.json`.
fn kind_name(kind: RLayerKind) -> &'static str {
    match kind {
        RLayerKind::Vector => "vector",
        RLayerKind::Table => "table",
        RLayerKind::Raster => "raster",
    }
}

/// The file extension for a layer: `fgb` for vectors, `csv` for tables, `tif` for rasters. A
/// format that does not match the kind, or is not one of the three, is an error.
fn ext_for(kind: RLayerKind, format: &str) -> Result<&'static str, String> {
    let expected = match kind {
        RLayerKind::Vector => "fgb",
        RLayerKind::Table => "csv",
        RLayerKind::Raster => "tif",
    };
    match format {
        "fgb" | "csv" | "tif" if format == expected => Ok(expected),
        "fgb" | "csv" | "tif" => Err(format!("a {} layer needs the format {expected}, not {format}", kind_name(kind))),
        _ => Err(format!("unknown layer format {format}; use fgb, csv or tif")),
    }
}

/// Refuses a layer larger than `cap` bytes.
pub(crate) fn check_layer_size(name: &str, len: usize, cap: usize) -> Result<(), String> {
    if len > cap {
        return Err(format!("layer {name} is larger than the {cap}-byte limit"));
    }
    Ok(())
}

/// The host rows (0-based) as the 1-based, sorted, deduplicated rows `params.json` holds. A row
/// too large to count from 1 is an error.
pub(crate) fn selection_rows(rows: &[usize]) -> Result<Vec<usize>, String> {
    let mut rows = rows.to_vec();
    rows.sort_unstable();
    rows.dedup();
    rows.iter().map(|&row| row.checked_add(1).ok_or_else(|| "a selected row is too large".to_string())).collect()
}
