// End-to-end tests: the real bootstrap.R under Rscript. Each test skips when Rscript, jsonlite
// or sf is missing.

use crate::RscriptRunner;
use crate::job::build_job_files;
use crate::manifest::convert_output;
use crate::stage::stage_dataset;
use crate::wire::{RExecutionInput, RExecutionOutput, RFile};
use rustgis_analysis::{Output, OutputLayer, ToolResult};
use rustgis_data::{Coord, Dataset, Feature, Field, FieldType, Geometry, GeometryType, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// How long one script run may take before it is killed.
const RUN_LIMIT: Duration = Duration::from_secs(60);

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

/// A job directory under the system temp dir, named by process and test. Removed on drop.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(test: &str) -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("rustgis-r-harness-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
fn cities_rows(name: &str) -> Option<usize> {
    (name == "Cities").then_some(3)
}

/// Writes the job files, runs bootstrap.R in `dir`, and reads back the files under `out/`.
fn run_job(input: &RExecutionInput, dir: &Path) -> Result<RExecutionOutput, String> {
    let files = build_job_files(input)?;
    for file in &files {
        let path = dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, &file.data).map_err(|e| e.to_string())?;
    }

    // Output goes to a file, not a pipe, so the child never blocks on a full pipe.
    let log = std::fs::File::create(dir.join("rscript.log")).map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let program = RscriptRunner.locate().map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(["--no-save", "--no-restore", "--no-init-file", "bootstrap.R"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => break,
            None if start.elapsed() < RUN_LIMIT => std::thread::sleep(Duration::from_millis(20)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the R script ran for more than 60 seconds".into());
            }
        }
    }

    let mut out_files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("out")) {
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let data = std::fs::read(entry.path()).map_err(|e| e.to_string())?;
            out_files.push(RFile { path: format!("out/{name}"), data });
        }
    }
    out_files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(RExecutionOutput { files: out_files, ..Default::default() })
}

/// Runs one case in its own scratch dir and converts the output.
fn run_case(test: &str, input: &RExecutionInput) -> Result<ToolResult, String> {
    let scratch = ScratchDir::new(test).map_err(|e| e.to_string())?;
    let out = run_job(input, scratch.path())?;
    convert_output(&out, &cities_rows)
}

#[test]
fn harness_runs_script_and_returns_layers() -> TestResult {
    if skip_without_r() {
        return Ok(());
    }
    let input = RExecutionInput {
        script: r#"
cities <- rgis_read_layer("Cities")
buffered <- sf::st_buffer(cities, 0.01)
rgis_write_layer(buffered, "Buffered")
rgis_write_table(sf::st_drop_geometry(buffered), "Stats")
rgis_log("done")
"#
        .into(),
        layers: vec![stage_dataset(&point_dataset(), &[])?],
        ..Default::default()
    };
    let result = run_case("harness_runs_script_and_returns_layers", &input)?;
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
    let input = RExecutionInput {
        script: r#"rgis_select("Cities", c(1, 2))"#.into(),
        layers: vec![stage_dataset(&point_dataset(), &[])?],
        ..Default::default()
    };
    let result = run_case("harness_select_returns_zero_based_rows", &input)?;
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
    let input = RExecutionInput { script: r#"stop("boom")"#.into(), ..Default::default() };
    let Err(err) = run_case("harness_script_error_is_failure", &input) else {
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
    let input = RExecutionInput {
        script: r#"
x <- sf::st_as_sf(data.frame(x = 500000, y = 4000000), coords = c("x", "y"))
rgis_write_layer(x, "Projected")
"#
        .into(),
        ..Default::default()
    };
    let Err(err) = run_case("harness_projected_without_crs_is_refused", &input) else {
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
    let input = RExecutionInput {
        script: r#"
n <- nrow(rgis_read_layer("Cities"))
rgis_log(paste("rows", n))
rgis_write_table(data.frame(n = n), "Count")
"#
        .into(),
        // Row 1 counted from 0, so the script sees row 2 of three.
        layers: vec![stage_dataset(&point_dataset(), &[1])?],
        ..Default::default()
    };
    let result = run_case("harness_staged_selection_is_visible", &input)?;
    let Output::Layers(layers) = &result.output else {
        return Err("expected layers".into());
    };
    assert!(matches!(layers.as_slice(), [OutputLayer::Table(_)]));
    assert!(result.messages.iter().any(|m| m == "rows 1"), "{:?}", result.messages);
    Ok(())
}
