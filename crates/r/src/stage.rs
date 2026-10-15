//! The layer stagers: turn host data into the layers a script is given.

use crate::wire::{MAX_FILE_BYTES, RLayerInput, RLayerKind};
use rustgis_data::raster::Raster;
use rustgis_data::{Dataset, GeometryType, io, source::fgb};

/// Stages a dataset as a layer: features as FlatGeobuf, a table with no geometry as CSV.
/// `selection` holds host rows counted from 0; see [`clean_selection`].
pub fn stage_dataset(ds: &Dataset, selection: &[usize]) -> Result<RLayerInput, String> {
    let (kind, format, data) = if ds.geometry_type == GeometryType::None {
        let text = io::to_csv(ds).map_err(|e| format!("layer '{}' could not be staged: {e}", ds.name))?;
        (RLayerKind::Table, "csv", text.into_bytes())
    } else {
        let bytes = fgb::write(ds).map_err(|e| format!("layer '{}' could not be staged: {e}", ds.name))?;
        (RLayerKind::Vector, "fgb", bytes)
    };
    check_layer_size(&ds.name, data.len(), MAX_FILE_BYTES)?;
    Ok(RLayerInput { name: ds.name.clone(), format: format.into(), data, kind, selection: clean_selection(selection, ds.features.len()) })
}

/// Stages a raster as a GeoTIFF layer. The original bytes go through when the file name ends in
/// `.tif` or `.tiff`; any other raster is written again as a GeoTIFF.
pub fn stage_raster(name: &str, original: Option<(&str, &[u8])>, raster: &Raster) -> Result<RLayerInput, String> {
    let data = match original {
        Some((file_name, bytes)) if is_tiff_name(file_name) => bytes.to_vec(),
        _ => rustgis_data::raster::geotiff::write(raster, 0).map_err(|e| format!("layer '{name}' could not be staged: {e}"))?,
    };
    check_layer_size(name, data.len(), MAX_FILE_BYTES)?;
    Ok(RLayerInput { name: name.into(), format: "tif".into(), data, kind: RLayerKind::Raster, selection: Vec::new() })
}

/// Whether a file name ends in `.tif` or `.tiff`, in any letter case.
fn is_tiff_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".tif") || lower.ends_with(".tiff")
}

/// Fails when a staged layer is larger than `cap` bytes.
pub(crate) fn check_layer_size(name: &str, len: usize, cap: usize) -> Result<(), String> {
    if len > cap { Err(format!("layer '{name}' is too large to send to R")) } else { Ok(()) }
}

/// The selection sorted, without repeats, and without rows past `len` (the features there are).
fn clean_selection(selection: &[usize], len: usize) -> Vec<usize> {
    let mut rows: Vec<usize> = selection.iter().copied().filter(|&row| row < len).collect();
    rows.sort_unstable();
    rows.dedup();
    rows
}
