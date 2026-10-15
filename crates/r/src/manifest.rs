//! The manifest reader: parses `out/manifest.json`, the file a script writes to say what it
//! produced, and converts it to a tool result. The file and everything in it are hostile input,
//! so every failure is a plain-English message, and a panic in the conversion is caught too.

use crate::wire::{MAX_FILE_BYTES, MAX_FILES, MAX_MANIFEST_BYTES, MAX_MESSAGES, RExecutionOutput, RFile};
use rustgis_analysis::{Output, OutputLayer, ToolResult};
use rustgis_data::source;
use rustgis_data::{Dataset, GeometryType, io, raster};
use serde_json::Value;
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The path of the result file a script writes.
const MANIFEST_PATH: &str = "out/manifest.json";

/// The longest message text kept, in bytes.
const MAX_MESSAGE_BYTES: usize = 4 << 10;

/// Converts a script's output to a tool result. `rows_of` gives the host rows an input layer was
/// read from, by name, or `None` when there is no such input. Any failure is returned as its message.
pub fn convert_output(out: &RExecutionOutput, rows_of: &dyn Fn(&str) -> Option<Vec<usize>>) -> Result<ToolResult, String> {
    match catch_unwind(AssertUnwindSafe(|| convert_manifest(out, rows_of))) {
        Ok(result) => result,
        Err(_) => Err("the R result could not be read".into()),
    }
}

fn convert_manifest(out: &RExecutionOutput, rows_of: &dyn Fn(&str) -> Option<Vec<usize>>) -> Result<ToolResult, String> {
    let file = find_file(&out.files, MANIFEST_PATH).ok_or("the R script did not finish: no result file was written")?;
    if file.data.len() > MAX_MANIFEST_BYTES {
        return Err("the R result file is too large".into());
    }
    let manifest: Value = serde_json::from_slice(&file.data).map_err(|e| format!("the R result file is not valid JSON: {e}"))?;
    if manifest.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("the R result file has an unsupported version".into());
    }
    let ok = manifest.get("ok").and_then(Value::as_bool).ok_or("the R result file has no ok flag")?;
    if !ok {
        let error = manifest.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()).unwrap_or("the R script failed");
        return Err(error.to_string());
    }

    let layers: &[Value] = match manifest.get("layers") {
        None | Some(Value::Null) => &[],
        Some(Value::Array(list)) => list,
        Some(_) => return Err("the R result's layers must be a list".into()),
    };
    let selection = manifest.get("selection").filter(|s| !s.is_null());
    if layers.is_empty() == selection.is_none() {
        return Err("the R result must list layers or a selection, not both and not neither".into());
    }
    let mut messages = convert_messages(manifest.get("messages"))?;

    let output = match selection {
        Some(sel) => convert_selection(sel, rows_of)?,
        None => {
            if layers.len() > MAX_FILES {
                return Err(format!("the R script wrote more than {MAX_FILES} layers"));
            }
            let mut converted = Vec::with_capacity(layers.len());
            for entry in layers {
                converted.push(convert_layer(entry, &out.files)?);
            }
            Output::Layers(converted)
        }
    };
    messages.extend(convert_printed(&out.stdout, MAX_MESSAGES.saturating_sub(messages.len())));
    Ok(ToolResult { output, messages })
}

/// The last `room` non-blank printed lines, each cut to `MAX_MESSAGE_BYTES`. The oldest go first.
fn convert_printed(stdout: &str, room: usize) -> Vec<String> {
    let lines: Vec<&str> = stdout.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect();
    lines.iter().rev().take(room).rev().map(|line| truncate_text(line, MAX_MESSAGE_BYTES).to_string()).collect()
}

/// The file at `path` among a script's output files.
fn find_file<'a>(files: &'a [RFile], path: &str) -> Option<&'a RFile> {
    files.iter().find(|f| f.path == path)
}

/// Reads one `layers[]` entry: its name, kind, format and file.
fn convert_layer(entry: &Value, files: &[RFile]) -> Result<OutputLayer, String> {
    let name = entry.get("name").and_then(Value::as_str).ok_or("each R result layer needs a name")?;
    let kind = entry.get("kind").and_then(Value::as_str).unwrap_or("");
    let format = entry.get("format").and_then(Value::as_str).unwrap_or("");
    let path = entry.get("file").and_then(Value::as_str).ok_or_else(|| format!("layer '{name}' needs a file"))?;

    check_layer_path(path).map_err(|e| format!("layer '{name}': {e}"))?;
    let extension = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    if extension.as_deref() != Some(format) {
        return Err(format!("layer '{name}': its file does not match its format '{format}'"));
    }
    let file = find_file(files, path).ok_or_else(|| format!("layer '{name}': its file '{path}' was not written"))?;
    if file.data.len() > MAX_FILE_BYTES {
        return Err(format!("layer '{name}' is too large"));
    }
    let bytes = &file.data;

    match (kind, format) {
        ("vector", "fgb" | "geojson") => {
            let mut ds = read_vector(path, format, bytes, name)?;
            if ds.geometry_type == GeometryType::None {
                return Err(format!("layer '{name}' is a vector layer with no geometry"));
            }
            check_coords(name, &ds)?;
            ds.name = name.to_string();
            Ok(OutputLayer::Vector(ds))
        }
        ("table", "csv") => {
            let text = std::str::from_utf8(bytes).map_err(|_| format!("layer '{name}' is not UTF-8 text"))?;
            let (ds, report) = io::read_csv(text, name).map_err(|e| format!("layer '{name}' could not be read: {e}"))?;
            skipped_check(name, report.skipped)?;
            Ok(OutputLayer::Table(ds))
        }
        ("raster", "tif") => {
            raster::read_bytes(path, bytes).map_err(|e| format!("layer '{name}' could not be read: {e}"))?;
            Ok(OutputLayer::Raster { name: name.to_string(), file_name: path.to_string(), bytes: bytes.clone() })
        }
        _ => Err(format!("layer '{name}' has an unknown kind '{kind}' and format '{format}'")),
    }
}

/// A layer with unreadable features is damaged: the reader would keep the rest, so refuse it.
fn skipped_check(name: &str, skipped: usize) -> Result<(), String> {
    if skipped > 0 {
        return Err(format!("layer '{name}' is damaged: {skipped} features could not be read"));
    }
    Ok(())
}

/// Reads a vector layer. A FlatGeobuf header says how many features it holds, and the reader
/// stops quietly at the end of a cut-short file, so the count is checked against it.
fn read_vector(path: &str, format: &str, bytes: &[u8], name: &str) -> Result<Dataset, String> {
    let fail = |e: rustgis_data::DataError| format!("layer '{name}' could not be read: {e}");
    if format != "fgb" {
        let (ds, report) = io::read_bytes(path, bytes).map_err(fail)?;
        skipped_check(name, report.skipped)?;
        return Ok(ds);
    }
    let source = source::open_bytes(path, bytes.to_vec(), None).map_err(fail)?;
    let expected = source.info().count;
    let (ds, report) = source::materialize(source.as_ref()).map_err(fail)?;
    skipped_check(name, report.skipped)?;
    if let Some(count) = expected
        && u64::try_from(ds.features.len()).unwrap_or(u64::MAX) < count
    {
        return Err(format!("layer '{name}' could not be read in full: {} of {count} features", ds.features.len()));
    }
    Ok(ds)
}

/// Only `out/<name>` is allowed: one level, no `..`, no `\`, no absolute path.
fn check_layer_path(path: &str) -> Result<(), String> {
    let name = path.strip_prefix("out/").ok_or("its file must be under out/")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("its file path '{path}' is not allowed"));
    }
    Ok(())
}

/// Checks that every coordinate is a longitude and latitude (inputs are in EPSG:4326).
pub(crate) fn check_coords(name: &str, ds: &Dataset) -> Result<(), String> {
    let mut bad = false;
    for feature in &ds.features {
        if let Some(geometry) = &feature.geometry {
            geometry.for_each_coord(|c| {
                let lon_ok = c.x.is_finite() && (-180.0..=180.0).contains(&c.x);
                let lat_ok = c.y.is_finite() && (-90.0..=90.0).contains(&c.y);
                if !(lon_ok && lat_ok) {
                    bad = true;
                }
            });
        }
    }
    if bad {
        return Err(format!("layer '{name}' has coordinates outside longitude -180 to 180 and latitude -90 to 90"));
    }
    Ok(())
}

/// Reads the `selection` object: 1-based rows of an input layer, mapped to the host rows the
/// script read, sorted.
fn convert_selection(sel: &Value, rows_of: &dyn Fn(&str) -> Option<Vec<usize>>) -> Result<Output, String> {
    let layer = sel.get("layer").and_then(Value::as_str).ok_or("the selection needs a layer name")?;
    let read_rows = rows_of(layer).ok_or_else(|| format!("the selection names layer '{layer}', which is not an input"))?;
    let rows = sel.get("rows").and_then(Value::as_array).ok_or("the selection rows must be a list")?;
    let mut picked = BTreeSet::new();
    for row in rows {
        let number = row.as_u64().ok_or_else(|| format!("selection row {row} is not a whole number"))?;
        let index = usize::try_from(number).map_err(|_| format!("selection row {number} is out of range"))?;
        let host = index
            .checked_sub(1)
            .and_then(|i| read_rows.get(i))
            .ok_or_else(|| format!("selection row {index} is outside the {} rows read from layer '{layer}'", read_rows.len()))?;
        picked.insert(*host);
    }
    Ok(Output::Selection { layer: layer.to_string(), rows: picked.into_iter().collect() })
}

/// Reads the `messages` list: at most `MAX_MESSAGES`, each text cut to `MAX_MESSAGE_BYTES`.
/// Warnings and errors get a prefix; any other level is plain info.
fn convert_messages(list: Option<&Value>) -> Result<Vec<String>, String> {
    let items: &[Value] = match list {
        None | Some(Value::Null) => &[],
        Some(Value::Array(items)) => items,
        Some(_) => return Err("the R result's messages must be a list".into()),
    };
    let mut messages = Vec::new();
    for item in items.iter().take(MAX_MESSAGES) {
        let text = item.get("text").and_then(Value::as_str).ok_or("each R result message needs a text")?;
        let prefix = match item.get("level").and_then(Value::as_str) {
            Some("warn") => "warning: ",
            Some("error") => "error: ",
            _ => "",
        };
        messages.push(format!("{prefix}{}", truncate_text(text, MAX_MESSAGE_BYTES)));
    }
    Ok(messages)
}

/// The longest prefix of `text` of at most `max` bytes that ends on a character boundary.
fn truncate_text(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while text.get(..end).is_none() {
        end -= 1;
    }
    text.get(..end).unwrap_or("")
}
