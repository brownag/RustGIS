// Layer stager tests.
use crate::stage::{check_layer_size, stage_dataset, stage_raster};
use crate::wire::RLayerKind;
use rustgis_data::raster::{ColorHint, Crs, GeoTransform, Raster};
use rustgis_data::{Coord, Dataset, Feature, Field, FieldType, Geometry, GeometryType, Value, io};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn point_dataset() -> Dataset {
    let features = (0..3u32)
        .map(|i| Feature {
            id: u64::from(i),
            geometry: Some(Geometry::Point(Coord { x: f64::from(i), y: 1.0 })),
            attrs: vec![Value::Text(format!("c{i}"))],
        })
        .collect();
    Dataset {
        name: "Cities".into(),
        geometry_type: GeometryType::Point,
        fields: vec![Field { name: "name".into(), ty: FieldType::Text, alias: None }],
        features,
        source: None,
        windowed: false,
    }
}

fn small_raster() -> Result<Raster, Box<dyn std::error::Error>> {
    let transform = GeoTransform { origin_x: 0.0, origin_y: 1.0, pixel_width: 0.25, pixel_height: -0.25 };
    Ok(Raster::new("s", 4, 4, vec![vec![1.0; 16]], transform, Crs::LonLat, None, ColorHint::Gray, "Float32")?)
}

#[test]
fn point_dataset_stages_as_fgb_and_reads_back() -> TestResult {
    let layer = stage_dataset(&point_dataset(), &[])?;
    assert_eq!(layer.kind, RLayerKind::Vector);
    assert_eq!(layer.format, "fgb");
    assert_eq!(layer.name, "Cities");
    let (back, _) = io::read_bytes("cities.fgb", &layer.data)?;
    assert_eq!(back.features.len(), 3);
    Ok(())
}

#[test]
fn polygon_dataset_stages_as_fgb_and_reads_back() -> TestResult {
    let ring = vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 }, Coord { x: 0.0, y: 0.0 }];
    let mut ds = point_dataset();
    ds.geometry_type = GeometryType::Polygon;
    for f in &mut ds.features {
        f.geometry = Some(Geometry::Polygon(vec![vec![ring.clone()]]));
    }
    let layer = stage_dataset(&ds, &[])?;
    assert_eq!(layer.format, "fgb");
    let (back, _) = io::read_bytes("areas.fgb", &layer.data)?;
    assert_eq!(back.features.len(), 3);
    Ok(())
}

#[test]
fn geometryless_dataset_stages_as_csv_with_header() -> TestResult {
    let mut ds = point_dataset();
    ds.geometry_type = GeometryType::None;
    for f in &mut ds.features {
        f.geometry = None;
    }
    let layer = stage_dataset(&ds, &[])?;
    assert_eq!(layer.kind, RLayerKind::Table);
    assert_eq!(layer.format, "csv");
    let text = String::from_utf8(layer.data)?;
    assert_eq!(text.lines().next(), Some("name"));
    Ok(())
}

#[test]
fn selection_is_sorted_deduped_and_clipped() -> TestResult {
    let layer = stage_dataset(&point_dataset(), &[2, 0, 2, 9])?;
    assert_eq!(layer.selection, vec![0, 2]);
    Ok(())
}

#[test]
fn layer_size_check_uses_the_cap_given() {
    assert_eq!(check_layer_size("Big", 11, 10), Err("layer 'Big' is too large to send to R".to_string()));
    assert!(check_layer_size("Ok", 10, 10).is_ok());
}

#[test]
fn raster_keeps_an_original_tif() -> TestResult {
    let raster = small_raster()?;
    let original = b"II*\0original".to_vec();
    let layer = stage_raster("Dem", Some(("dem.TIF", &original)), &raster)?;
    assert_eq!(layer.kind, RLayerKind::Raster);
    assert_eq!(layer.format, "tif");
    assert_eq!(layer.data, original);
    Ok(())
}

#[test]
fn raster_without_original_is_written_as_geotiff() -> TestResult {
    let raster = small_raster()?;
    let layer = stage_raster("Dem", None, &raster)?;
    assert_eq!(layer.data, rustgis_data::raster::geotiff::write(&raster, 0)?);
    Ok(())
}

#[test]
fn asc_original_is_not_passed_through() -> TestResult {
    let raster = small_raster()?;
    let layer = stage_raster("Dem", Some(("dem.asc", b"ncols 4")), &raster)?;
    assert_eq!(layer.data, rustgis_data::raster::geotiff::write(&raster, 0)?);
    Ok(())
}
