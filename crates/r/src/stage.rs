//! The layer stagers: turn host data into the layers a script is given.

use crate::job::check_layer_size;
use crate::wire::{MAX_FILE_BYTES, RLayerInput, RLayerKind};
use rustgis_data::raster::Raster;
use rustgis_data::{Dataset, GeometryType, io, source::fgb};

/// A staged layer and the host rows it holds. `read_rows` are the host rows the script reads,
/// in order: row `i` of the script's output is `read_rows[i - 1]`.
pub struct StagedLayer {
    pub layer: RLayerInput,
    pub read_rows: Vec<usize>,
}

/// Stages a dataset as a layer: features as FlatGeobuf, a table with no geometry as CSV.
/// `selection` holds host rows counted from 0; only the staged ones are kept.
pub fn stage_dataset(ds: &Dataset, selection: &[usize]) -> Result<StagedLayer, String> {
    let (kind, data, staged_rows): (RLayerKind, Vec<u8>, Vec<usize>) = if ds.geometry_type == GeometryType::None {
        let text = io::to_csv(ds).map_err(|e| format!("layer '{}' could not be staged: {e}", ds.name))?;
        (RLayerKind::Table, text.into_bytes(), (0..ds.features.len()).collect())
    } else {
        let bytes = fgb::write(ds).map_err(|e| format!("layer '{}' could not be staged: {e}", ds.name))?;
        // Features without a geometry are not written (see `fgb::write`).
        let rows = ds.features.iter().enumerate().filter(|(_, f)| f.geometry.is_some()).map(|(i, _)| i).collect();
        (RLayerKind::Vector, bytes, rows)
    };
    check_layer_size(&ds.name, data.len(), MAX_FILE_BYTES)?;
    let (selection, read_rows) = map_selection(selection, &staged_rows);
    let layer = RLayerInput { name: ds.name.clone(), data, kind, selection };
    Ok(StagedLayer { layer, read_rows })
}

/// Stages a raster as a GeoTIFF layer. The original bytes go through when the file name ends in
/// `.tif` or `.tiff`; any other raster is written again as a GeoTIFF.
pub fn stage_raster(name: &str, original: Option<(&str, &[u8])>, raster: &Raster) -> Result<StagedLayer, String> {
    let data = match original {
        Some((file_name, bytes)) if is_tiff_name(file_name) => bytes.to_vec(),
        _ => rustgis_data::raster::geotiff::write(raster, 0).map_err(|e| format!("layer '{name}' could not be staged: {e}"))?,
    };
    check_layer_size(name, data.len(), MAX_FILE_BYTES)?;
    let layer = RLayerInput { name: name.into(), data, kind: RLayerKind::Raster, selection: Vec::new() };
    Ok(StagedLayer { layer, read_rows: Vec::new() })
}

/// Whether a file name ends in `.tif` or `.tiff`, in any letter case.
fn is_tiff_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".tif") || lower.ends_with(".tiff")
}

/// Maps a host selection to the staged layer. `staged_rows` is sorted. Returns the positions of
/// the selected rows within `staged_rows`, and the rows the script reads: the selected rows, or
/// all staged rows when none is selected. Rows not staged are dropped.
fn map_selection(selection: &[usize], staged_rows: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let mut picked: Vec<usize> = selection.iter().copied().filter(|row| staged_rows.binary_search(row).is_ok()).collect();
    picked.sort_unstable();
    picked.dedup();
    let positions: Vec<usize> = picked.iter().filter_map(|row| staged_rows.binary_search(row).ok()).collect();
    let read_rows = if picked.is_empty() { staged_rows.to_vec() } else { picked };
    (positions, read_rows)
}
