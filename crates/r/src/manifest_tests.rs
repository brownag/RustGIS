use crate::RRunnerError;
use crate::manifest::{check_coords, convert_output};
use crate::wire::{MAX_MESSAGES, RExecutionOutput, RFile};
use rustgis_analysis::{Output, OutputLayer};
use rustgis_data::raster::{ColorHint, Crs, GeoTransform, Raster, geotiff};
use rustgis_data::source::fgb;
use rustgis_data::{Coord, Dataset, Feature, Geometry, GeometryType};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Input layer "Cities" was read from five host rows; any other name is not an input.
fn cities_rows(name: &str) -> Option<Vec<usize>> {
    (name == "Cities").then(|| (0..5).collect())
}

fn cities() -> Dataset {
    let mut ds = Dataset::new("Cities", GeometryType::Point, vec![]);
    ds.features.push(Feature { id: 1, geometry: Some(Geometry::Point(Coord { x: 1.0, y: 2.0 })), attrs: vec![] });
    ds
}

fn file(path: &str, data: Vec<u8>) -> RFile {
    RFile { path: path.into(), data }
}

/// The three layer files the standard manifest refers to: a vector, a table and a raster.
fn good_files() -> Result<Vec<RFile>, Box<dyn std::error::Error>> {
    let transform = GeoTransform { origin_x: 0.0, origin_y: 1.0, pixel_width: 0.25, pixel_height: -0.25 };
    let raster = Raster::new("dem", 4, 4, vec![vec![1.0; 16]], transform, Crs::LonLat, None, ColorHint::Gray, "Float32")?;
    Ok(vec![
        file("out/0.fgb", fgb::write(&cities())?),
        file("out/1.csv", b"name,score\nalpha,3\nbeta,4\n".to_vec()),
        file("out/2.tif", geotiff::write(&raster, 0)?),
    ])
}

/// An output whose result file holds `manifest` and whose other files are `extra`.
fn manifest_out(manifest: &Value, extra: Vec<RFile>) -> RExecutionOutput {
    let mut files = vec![file("out/manifest.json", manifest.to_string().into_bytes())];
    files.extend(extra);
    RExecutionOutput { files, ..Default::default() }
}

/// An output with only a manifest and no other files.
fn bare(manifest: &Value) -> RExecutionOutput {
    manifest_out(manifest, Vec::new())
}

fn standard_manifest() -> Value {
    json!({
        "version": 1,
        "ok": true,
        "error": null,
        "layers": [
            {"name": "Hot spots", "file": "out/0.fgb", "kind": "vector", "format": "fgb"},
            {"name": "Stats", "file": "out/1.csv", "kind": "table", "format": "csv"},
            {"name": "Dem", "file": "out/2.tif", "kind": "raster", "format": "tif"}
        ],
        "messages": [{"level": "info", "text": "done"}, {"level": "warn", "text": "careful"}]
    })
}

/// The message of a failure (a script's own error text, or the runner's text for a notice).
fn text(error: RRunnerError) -> String {
    match error {
        RRunnerError::Failed(message) => message,
        other => other.to_string(),
    }
}

/// The error message of a conversion that must fail.
fn failure(out: &RExecutionOutput) -> Result<String, Box<dyn std::error::Error>> {
    match convert_output(out, &cities_rows) {
        Ok(_) => Err("expected the conversion to fail".into()),
        Err(error) => Ok(text(error)),
    }
}

#[test]
fn layers_round_trip() -> TestResult {
    let out = manifest_out(&standard_manifest(), good_files()?);
    let result = convert_output(&out, &cities_rows)?;
    let Output::Layers(layers) = result.output else {
        return Err("expected layers".into());
    };
    let [vector, table, raster] = layers.as_slice() else {
        return Err("expected three layers".into());
    };
    match vector {
        OutputLayer::Vector(ds) => assert_eq!((ds.name.as_str(), ds.features.len()), ("Hot spots", 1)),
        _ => return Err("expected a vector layer".into()),
    }
    match table {
        OutputLayer::Table(ds) => assert_eq!((ds.name.as_str(), ds.features.len()), ("Stats", 2)),
        _ => return Err("expected a table".into()),
    }
    match raster {
        OutputLayer::Raster { name, file_name, bytes } => {
            assert_eq!((name.as_str(), file_name.as_str()), ("Dem", "out/2.tif"));
            assert!(!bytes.is_empty());
        }
        _ => return Err("expected a raster".into()),
    }
    assert_eq!(result.messages, vec!["done".to_string(), "warning: careful".to_string()]);
    Ok(())
}

#[test]
fn selection_is_one_based_sorted_and_unique() -> TestResult {
    let manifest = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [3, 1, 3, 5]}});
    let result = convert_output(&bare(&manifest), &cities_rows)?;
    let Output::Selection { layer, rows } = result.output else {
        return Err("expected a selection".into());
    };
    assert_eq!(layer, "Cities");
    assert_eq!(rows, vec![0, 2, 4]);
    Ok(())
}

#[test]
fn selection_rows_map_through_the_rows_read() -> TestResult {
    // The script read host rows 0 and 2 only: its row 2 is host row 2.
    let rows_of = |name: &str| (name == "Cities").then(|| vec![0, 2]);
    let manifest = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [2]}});
    let result = convert_output(&bare(&manifest), &rows_of)?;
    let Output::Selection { rows, .. } = result.output else {
        return Err("expected a selection".into());
    };
    assert_eq!(rows, vec![2]);
    // Row 3 is past the two rows read.
    let past = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [3]}});
    let Err(error) = convert_output(&bare(&past), &rows_of) else {
        return Err("expected the conversion to fail".into());
    };
    assert_eq!(text(error), "selection row 3 is outside the 2 rows read from layer 'Cities'");
    // Row 0 is not a row.
    let zero = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [0]}});
    assert!(convert_output(&bare(&zero), &rows_of).is_err());
    Ok(())
}

#[test]
fn failed_script_returns_its_error() -> TestResult {
    let manifest = json!({"version": 1, "ok": false, "error": "boom"});
    assert_eq!(failure(&bare(&manifest))?, "boom");
    let generic = json!({"version": 1, "ok": false, "error": null});
    assert_eq!(failure(&bare(&generic))?, "the R script failed");
    Ok(())
}

#[test]
fn a_typed_notice_from_the_script_is_kept() -> TestResult {
    let manifest = json!({"version": 1, "ok": false, "error": "english", "notice": "need_sf"});
    let Err(error) = convert_output(&bare(&manifest), &cities_rows) else {
        return Err("expected the conversion to fail".into());
    };
    assert_eq!(error, RRunnerError::Notice(crate::RNotice::NeedSf));
    // An unknown or non-text code is not a notice: the message is used instead.
    let unknown = json!({"version": 1, "ok": false, "error": "boom", "notice": "bogus"});
    assert_eq!(failure(&bare(&unknown))?, "boom");
    let numeric = json!({"version": 1, "ok": false, "error": "boom", "notice": 3});
    assert_eq!(failure(&bare(&numeric))?, "boom");
    Ok(())
}

#[test]
fn missing_result_file_is_a_failure() -> TestResult {
    let out = RExecutionOutput::default();
    assert!(failure(&out)?.contains("no result file was written"));
    Ok(())
}

#[test]
fn header_and_shape_errors() -> TestResult {
    let cases = [
        json!({"ok": true, "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 2, "ok": true, "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": "yes", "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": true}),
        json!({"version": 1, "ok": true, "layers": [{"name": "Hot spots", "file": "out/0.fgb", "kind": "vector", "format": "fgb"}],
               "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": true, "selection": {"layer": "Nope", "rows": [1]}}),
    ];
    for case in &cases {
        failure(&bare(case)).map_err(|_| format!("expected a failure for {case}"))?;
    }
    Ok(())
}

#[test]
fn layer_path_and_format_rules() -> TestResult {
    for path in ["out/../x.fgb", "/etc/passwd", "out\\x.fgb", "out/a/b.fgb", "out/", "x/0.fgb", "out/0.fgb/"] {
        let manifest = json!({"version": 1, "ok": true, "layers": [
            {"name": "A", "file": path, "kind": "vector", "format": "fgb"}]});
        let out = manifest_out(&manifest, vec![file(path, fgb::write(&cities())?)]);
        failure(&out).map_err(|_| format!("expected {path} to be refused"))?;
    }
    // The file's extension must match its format.
    let mismatch = json!({"version": 1, "ok": true, "layers": [
        {"name": "A", "file": "out/0.fgb", "kind": "vector", "format": "geojson"}]});
    assert!(failure(&manifest_out(&mismatch, vec![file("out/0.fgb", fgb::write(&cities())?)]))?.contains("does not match"));
    // A listed file that was never written.
    let missing = json!({"version": 1, "ok": true, "layers": [
        {"name": "A", "file": "out/9.fgb", "kind": "vector", "format": "fgb"}]});
    assert!(failure(&bare(&missing))?.contains("was not written"));
    // An unknown kind.
    let unknown = json!({"version": 1, "ok": true, "layers": [
        {"name": "A", "file": "out/0.fgb", "kind": "cube", "format": "fgb"}]});
    assert!(failure(&manifest_out(&unknown, vec![file("out/0.fgb", fgb::write(&cities())?)]))?.contains("unknown kind"));
    Ok(())
}

#[test]
fn coordinates_must_be_lon_lat() -> TestResult {
    for x in [f64::NAN, f64::INFINITY, 200.0] {
        let mut ds = cities();
        ds.features = vec![Feature { id: 1, geometry: Some(Geometry::Point(Coord { x, y: 2.0 })), attrs: vec![] }];
        assert!(check_coords("Bad", &ds).is_err(), "{x}");
    }
    assert!(check_coords("Good", &cities()).is_ok());
    Ok(())
}

#[test]
fn selection_rows_are_checked() -> TestResult {
    let bad_rows = [json!([-1]), json!([0]), json!([1.5]), json!([1e30]), json!(["x"]), json!([null]), json!([6])];
    for rows in bad_rows {
        let manifest = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": rows}});
        failure(&bare(&manifest)).map_err(|_| format!("expected a failure for rows {rows}"))?;
    }
    Ok(())
}

#[test]
fn messages_are_prefixed_capped_and_cut() -> TestResult {
    let long = "é".repeat(3000);
    let mut many = vec![json!({"level": "debug", "text": "d"})];
    many.extend((0..MAX_MESSAGES + 500).map(|_| json!({"level": "error", "text": "e"})));
    many.push(json!({"level": "warn", "text": long}));
    let manifest = json!({"version": 1, "ok": true, "messages": many, "selection": {"layer": "Cities", "rows": [1]}});
    let result = convert_output(&bare(&manifest), &cities_rows)?;
    assert_eq!(result.messages.len(), MAX_MESSAGES);
    assert_eq!(result.messages.first().map(String::as_str), Some("d"));
    assert_eq!(result.messages.get(1).map(String::as_str), Some("error: e"));
    // A short manifest that cuts a long text returns the prefix, still valid UTF-8.
    let cut = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [1]},
        "messages": [{"level": "warn", "text": "é".repeat(3000)}]});
    let kept = convert_output(&bare(&cut), &cities_rows)?;
    let text = kept.messages.first().ok_or("no message")?;
    assert!(text.starts_with("warning: é") && text.len() <= "warning: ".len() + 4096);
    Ok(())
}

#[test]
fn hostile_manifests_never_panic() -> TestResult {
    let junk = [
        json!("not an object"),
        json!(null),
        json!([]),
        json!({"version": "1", "ok": true}),
        json!({"version": 1, "ok": true, "layers": 5}),
        json!({"version": 1, "ok": true, "layers": [5]}),
        json!({"version": 1, "ok": true, "layers": [{"name": 1}]}),
        json!({"version": 1, "ok": true, "layers": [{"name": "a", "file": "out/0.fgb", "kind": "vector", "format": "fgb"}]}),
        json!({"version": 1, "ok": true, "layers": [{"name": "a", "file": 9, "kind": "vector", "format": "fgb"}]}),
        json!({"version": 1, "ok": true, "messages": 5, "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": true, "messages": [5], "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": true, "messages": [{"text": 5}], "selection": {"layer": "Cities", "rows": [1]}}),
        json!({"version": 1, "ok": true, "selection": "x"}),
        json!({"version": 1, "ok": true, "selection": {"layer": 5, "rows": [1]}}),
        json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": "x"}}),
        json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [1000000001]}}),
    ];
    for manifest in &junk {
        failure(&bare(manifest)).map_err(|_| format!("expected a failure for {manifest}"))?;
    }
    // Raw text the JSON reader must reject: deep nesting, truncation and NaN.
    let raw: [Vec<u8>; 3] =
        ["[".repeat(100_000).into_bytes(), b"{\"version\":1,\"ok\":true,".to_vec(), b"{\"version\":1,\"ok\":NaN}".to_vec()];
    for bytes in raw {
        let out = RExecutionOutput { files: vec![file("out/manifest.json", bytes)], ..Default::default() };
        assert!(convert_output(&out, &cities_rows).is_err());
    }
    // Truncated and non-UTF-8 layer bytes.
    let fgb_bytes = fgb::write(&cities())?;
    let half = fgb_bytes[..fgb_bytes.len() / 2].to_vec();
    let fgb_layer = json!({"version": 1, "ok": true, "layers": [{"name": "A", "file": "out/0.fgb", "kind": "vector", "format": "fgb"}]});
    assert!(convert_output(&manifest_out(&fgb_layer, vec![file("out/0.fgb", half)]), &cities_rows).is_err());
    let csv_layer = json!({"version": 1, "ok": true, "layers": [{"name": "A", "file": "out/0.csv", "kind": "table", "format": "csv"}]});
    assert!(convert_output(&manifest_out(&csv_layer, vec![file("out/0.csv", vec![0xff, 0xfe, b'a'])]), &cities_rows).is_err());
    let tif_layer = json!({"version": 1, "ok": true, "layers": [{"name": "A", "file": "out/0.tif", "kind": "raster", "format": "tif"}]});
    assert!(convert_output(&manifest_out(&tif_layer, vec![file("out/0.tif", b"II*\0short".to_vec())]), &cities_rows).is_err());
    Ok(())
}

#[test]
fn a_million_rows_are_accepted_and_one_more_is_not() -> TestResult {
    let all: Vec<Value> = (1..=1_000_000u64).map(|n| json!(n)).collect();
    let big = |rows: Vec<Value>| json!({"version": 1, "ok": true, "selection": {"layer": "Big", "rows": rows}});
    let rows_of = |name: &str| (name == "Big").then(|| (0..1_000_000usize).collect::<Vec<usize>>());
    let result = convert_output(&bare(&big(all.clone())), &rows_of)?;
    let Output::Selection { rows, .. } = result.output else {
        return Err("expected a selection".into());
    };
    assert_eq!(rows.len(), 1_000_000);
    let mut over = all;
    over.push(json!(1_000_001u64));
    assert!(convert_output(&bare(&big(over)), &rows_of).is_err());
    Ok(())
}

#[test]
fn printed_lines_follow_the_manifest_messages() -> TestResult {
    let manifest = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [1]},
        "messages": [{"level": "info", "text": "msg1"}, {"level": "info", "text": "msg2"}]});
    let mut out = bare(&manifest);
    out.stdout = "a\n\n   \nb  \n".into();
    let result = convert_output(&out, &cities_rows)?;
    assert_eq!(result.messages, vec!["msg1", "msg2", "a", "b"]);
    Ok(())
}

#[test]
fn printed_lines_are_capped_oldest_first() -> TestResult {
    let manifest = json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [1]},
        "messages": [{"text": "m1"}, {"text": "m2"}, {"text": "m3"}]});
    let mut out = bare(&manifest);
    out.stdout = (0..MAX_MESSAGES + 10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    let result = convert_output(&out, &cities_rows)?;
    assert_eq!(result.messages.len(), MAX_MESSAGES);
    assert_eq!(result.messages.get(..3), Some(&["m1".to_string(), "m2".to_string(), "m3".to_string()][..]));
    assert_eq!(result.messages.get(3).map(String::as_str), Some("line 13"));
    assert_eq!(result.messages.last().map(String::as_str), Some("line 1009"));
    Ok(())
}

#[test]
fn long_printed_line_is_cut_on_a_character_boundary() -> TestResult {
    let mut out = bare(&json!({"version": 1, "ok": true, "selection": {"layer": "Cities", "rows": [1]}}));
    let long = "é".repeat(3000);
    out.stdout = long.clone();
    let result = convert_output(&out, &cities_rows)?;
    let text = result.messages.first().ok_or("no message")?;
    assert!(text.len() <= 4096 && long.starts_with(text.as_str()));
    assert!(text.chars().all(|c| c == 'é'));
    Ok(())
}

#[test]
fn failed_manifest_ignores_printed_lines() -> TestResult {
    let mut out = bare(&json!({"version": 1, "ok": false, "error": "boom"}));
    out.stdout = "printed".into();
    assert_eq!(failure(&out)?, "boom");
    Ok(())
}
