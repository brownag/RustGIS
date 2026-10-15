// End-to-end tests: the real bootstrap.R under Rscript. Each test skips when Rscript, jsonlite
// or sf is missing.

use crate::RScriptRunner;
use crate::RscriptRunner;
use crate::manifest::convert_output;
use crate::stage::stage_dataset;
use crate::wire::RExecutionInput;
use rustgis_analysis::{Output, OutputLayer, ToolResult};
use rustgis_data::{Coord, Dataset, Feature, Field, FieldType, Geometry, GeometryType, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Serializes the cases that run R. The timeout case compares the job folders in the temp dir
/// before and after its run, which another running case would change.
static R_RUNS: Mutex<()> = Mutex::new(());

/// Holds the R-run lock for the rest of a test.
pub(crate) fn lock_r() -> MutexGuard<'static, ()> {
    R_RUNS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether R with jsonlite and sf is available. Prints a skip message when it is not.
fn skip_without_r() -> bool {
    let Ok(program) = RscriptRunner.locate() else {
        println!("skipped: Rscript with jsonlite and sf not available");
        return true;
    };
    let ready = Command::new(program)
        .args(["-e", "quit(status = !all(c(requireNamespace(\"jsonlite\", quietly=TRUE), requireNamespace(\"sf\", quietly=TRUE))))"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !ready {
        println!("skipped: Rscript with jsonlite and sf not available");
    }
    !ready
}

/// Three points named "Cities", at lon 0, 1 and 2.
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

/// The row count of the one input the tests use: "Cities", with three rows.
fn cities_rows(name: &str) -> Option<Vec<usize>> {
    (name == "Cities").then(|| vec![0, 1, 2])
}

/// Runs the script with the native runner and converts its output.
fn run_case(input: &RExecutionInput) -> Result<ToolResult, String> {
    let out = RscriptRunner.run(input, &|_| {}).map_err(|e| e.to_string())?;
    convert_output(&out, &cities_rows)
}

/// The job folders this process has left in the temp dir.
fn job_folders() -> Vec<PathBuf> {
    let prefix = format!("rustgis-r-{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return Vec::new() };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)))
        .collect()
}

#[test]
fn harness_runs_script_and_returns_layers() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let input = RExecutionInput {
        script: r#"
cities <- rgis_read_layer("Cities")
buffered <- sf::st_buffer(cities, 0.01)
rgis_write_layer(buffered, "Buffered")
rgis_write_table(sf::st_drop_geometry(buffered), "Stats")
rgis_log("done")
"#
        .into(),
        layers: vec![stage_dataset(&point_dataset(), &[])?.layer],
        ..Default::default()
    };
    let result = run_case(&input)?;
    let Output::Layers(layers) = result.output else {
        return Err("expected layers".into());
    };
    assert!(matches!(layers.as_slice(), [OutputLayer::Vector(_), OutputLayer::Table(_)]));
    assert!(result.messages.iter().any(|m| m == "done"));
    Ok(())
}

#[test]
fn harness_select_returns_zero_based_rows() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let input = RExecutionInput {
        script: r#"rgis_select("Cities", c(1, 2))"#.into(),
        layers: vec![stage_dataset(&point_dataset(), &[])?.layer],
        ..Default::default()
    };
    let result = run_case(&input)?;
    let Output::Selection { layer, rows } = result.output else {
        return Err("expected a selection".into());
    };
    assert_eq!(layer, "Cities");
    assert_eq!(rows, vec![0, 1]);
    Ok(())
}

#[test]
fn harness_script_error_is_failure() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let input = RExecutionInput { script: r#"stop("boom")"#.into(), ..Default::default() };
    let Err(err) = run_case(&input) else {
        return Err("expected the script to fail".into());
    };
    assert!(err.contains("boom"), "{err}");
    Ok(())
}

#[test]
fn harness_projected_without_crs_is_refused() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let input = RExecutionInput {
        script: r#"
x <- sf::st_as_sf(data.frame(x = 500000, y = 4000000), coords = c("x", "y"))
rgis_write_layer(x, "Projected")
"#
        .into(),
        ..Default::default()
    };
    let Err(err) = run_case(&input) else {
        return Err("expected the script to fail".into());
    };
    assert!(err.contains("no CRS"), "{err}");
    Ok(())
}

#[test]
fn harness_staged_selection_is_visible() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let input = RExecutionInput {
        script: r#"
n <- nrow(rgis_read_layer("Cities"))
rgis_log(paste("rows", n))
rgis_write_table(data.frame(n = n), "Count")
"#
        .into(),
        // Row 1 counted from 0, so the script sees row 2 of three.
        layers: vec![stage_dataset(&point_dataset(), &[1])?.layer],
        ..Default::default()
    };
    let result = run_case(&input)?;
    let Output::Layers(layers) = &result.output else {
        return Err("expected layers".into());
    };
    assert!(matches!(layers.as_slice(), [OutputLayer::Table(_)]));
    assert!(result.messages.iter().any(|m| m == "rows 1"), "{:?}", result.messages);
    Ok(())
}

#[test]
fn harness_timeout_stops_the_script() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let _r = lock_r();
    let before = job_folders();
    let input = RExecutionInput { script: "Sys.sleep(30)".into(), timeout_s: Some(1), ..Default::default() };
    let Err(err) = run_case(&input) else {
        return Err("expected the script to be stopped".into());
    };
    assert!(err.contains("stopped after 1 s"), "{err}");
    assert_eq!(job_folders(), before, "the job folder was left behind");
    Ok(())
}
