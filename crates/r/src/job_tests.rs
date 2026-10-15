//! Tests for the job builder: the file list, `params.json`, validation and hostile input.

use crate::harness::BOOTSTRAP_R;
use crate::job::{build_job_files, check_layer_size, job_paths_ok, selection_rows};
use crate::wire::{MAX_FILES, MAX_SCRIPT_BYTES, RExecutionInput, RFile, RLayerInput, RLayerKind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn sample_input() -> RExecutionInput {
    RExecutionInput {
        script: "print(1)".into(),
        layers: vec![
            RLayerInput {
                name: "Cities".into(),
                format: "fgb".into(),
                data: b"fgb-bytes".to_vec(),
                kind: RLayerKind::Vector,
                selection: vec![4, 0, 4],
            },
            RLayerInput {
                name: "Stats".into(),
                format: "csv".into(),
                data: b"a,b\n1,2\n".to_vec(),
                kind: RLayerKind::Table,
                selection: vec![],
            },
        ],
        args: serde_json::json!({ "k": 3 }),
        timeout_s: None,
        lang: "fr".into(),
    }
}

fn file<'a>(files: &'a [RFile], path: &str) -> Result<&'a RFile, Box<dyn std::error::Error>> {
    files.iter().find(|f| f.path == path).ok_or_else(|| format!("no {path} in the job").into())
}

fn params_of(files: &[RFile]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&file(files, "params.json")?.data)?)
}

fn error_of(input: &RExecutionInput) -> Result<String, Box<dyn std::error::Error>> {
    build_job_files(input).err().ok_or_else(|| "expected an error".into())
}

fn one_layer(name: &str, format: &str, kind: RLayerKind) -> RExecutionInput {
    RExecutionInput {
        script: "1".into(),
        layers: vec![RLayerInput { name: name.into(), format: format.into(), kind, ..Default::default() }],
        ..Default::default()
    }
}

#[test]
fn file_list_and_order() -> TestResult {
    let files = build_job_files(&sample_input())?;
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["script.R", "bootstrap.R", "params.json", "in/0.fgb", "in/1.csv"]);
    Ok(())
}

#[test]
fn file_contents_are_the_inputs() -> TestResult {
    let files = build_job_files(&sample_input())?;
    assert_eq!(file(&files, "script.R")?.data, b"print(1)");
    assert_eq!(file(&files, "bootstrap.R")?.data, BOOTSTRAP_R.as_bytes());
    assert_eq!(file(&files, "in/0.fgb")?.data, b"fgb-bytes");
    assert_eq!(file(&files, "in/1.csv")?.data, b"a,b\n1,2\n");
    assert!(job_paths_ok(&files));
    Ok(())
}

#[test]
fn params_parse_back_with_one_based_selection() -> TestResult {
    let params = params_of(&build_job_files(&sample_input())?)?;
    assert_eq!(params["version"], 1);
    assert_eq!(params["lang"], "fr");
    assert_eq!(params["timeout_s"], 120);
    assert_eq!(params["args"], serde_json::json!({ "k": 3 }));
    assert_eq!(params["inputs"][0]["index"], 0);
    assert_eq!(params["inputs"][0]["name"], "Cities");
    assert_eq!(params["inputs"][0]["kind"], "vector");
    assert_eq!(params["inputs"][0]["format"], "fgb");
    assert_eq!(params["inputs"][0]["file"], "in/0.fgb");
    assert_eq!(params["inputs"][0]["selection"], serde_json::json!([1, 5]));
    assert_eq!(params["inputs"][1]["kind"], "table");
    assert_eq!(params["inputs"][1]["selection"], serde_json::json!([]));
    Ok(())
}

#[test]
fn timeout_lang_and_args_are_cleaned() -> TestResult {
    let mut input = sample_input();
    input.timeout_s = Some(0);
    input.lang = "bad lang!".into();
    input.args = serde_json::json!([1, 2]);
    let params = params_of(&build_job_files(&input)?)?;
    assert_eq!(params["timeout_s"], 1);
    assert_eq!(params["lang"], "en");
    assert_eq!(params["args"], serde_json::json!({}));

    input.timeout_s = Some(1_000_000);
    let params = params_of(&build_job_files(&input)?)?;
    assert_eq!(params["timeout_s"], 86_400);
    Ok(())
}

#[test]
fn selection_rows_are_sorted_deduped_and_one_based() -> TestResult {
    assert_eq!(selection_rows(&[4, 0, 4])?, vec![1, 5]);
    assert_eq!(selection_rows(&[])?, Vec::<usize>::new());
    assert!(selection_rows(&[usize::MAX]).is_err());
    let mut input = sample_input();
    input.layers[0].selection = vec![usize::MAX];
    assert!(error_of(&input)?.contains("too large"));
    Ok(())
}

#[test]
fn layer_size_is_checked_against_the_cap() -> TestResult {
    assert!(check_layer_size("Small", 10, 10).is_ok());
    let err = check_layer_size("Big", 11, 10).err().ok_or("expected an error")?;
    assert!(err.contains("Big") && err.contains("10-byte"));
    Ok(())
}

#[test]
fn empty_script_is_refused() -> TestResult {
    let mut input = sample_input();
    input.script = " \n\t ".into();
    assert_eq!(error_of(&input)?, "the script is empty");
    Ok(())
}

#[test]
fn oversize_script_is_refused() -> TestResult {
    let mut input = sample_input();
    input.script = "x".repeat(MAX_SCRIPT_BYTES + 1);
    assert!(error_of(&input)?.contains("larger than"));
    Ok(())
}

#[test]
fn too_many_layers_are_refused() -> TestResult {
    let mut input = sample_input();
    input.layers = (0..=MAX_FILES).map(|_| RLayerInput { format: "csv".into(), kind: RLayerKind::Table, ..Default::default() }).collect();
    assert!(error_of(&input)?.contains("layers"));
    input.layers.truncate(MAX_FILES);
    assert!(build_job_files(&input).is_ok());
    Ok(())
}

#[test]
fn kind_and_format_must_agree() -> TestResult {
    assert!(error_of(&one_layer("A", "csv", RLayerKind::Vector))?.contains("needs the format fgb"));
    assert!(error_of(&one_layer("A", "fgb", RLayerKind::Table))?.contains("needs the format csv"));
    assert!(error_of(&one_layer("A", "fgb", RLayerKind::Raster))?.contains("needs the format tif"));
    Ok(())
}

#[test]
fn unknown_format_is_refused() -> TestResult {
    assert!(error_of(&one_layer("A", "geojson", RLayerKind::Vector))?.contains("unknown layer format"));
    Ok(())
}

#[test]
fn hostile_layer_names_never_reach_a_path() -> TestResult {
    let names = ["../../etc".to_string(), String::new(), "数据層 🗺".to_string(), "x".repeat(10 * 1024)];
    for name in names {
        let files = build_job_files(&one_layer(&name, "fgb", RLayerKind::Vector))?;
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["script.R", "bootstrap.R", "params.json", "in/0.fgb"]);
        assert!(job_paths_ok(&files));
        let params = params_of(&files)?;
        assert_eq!(params["inputs"][0]["name"], name.as_str());
    }
    Ok(())
}

#[test]
fn job_paths_ok_refuses_unknown_paths() {
    let path_file = |path: &str| RFile { path: path.into(), data: Vec::new() };
    assert!(job_paths_ok(&[path_file("script.R"), path_file("in/12.tif")]));
    for bad in ["../x", "/abs", "in\\0.fgb", "out/0.fgb", "in/x.fgb", "in/0.fgb.fgb", "in/../0.fgb", "in/.fgb", ""] {
        assert!(!job_paths_ok(&[path_file(bad)]), "{bad} should be refused");
    }
}

#[test]
fn nul_and_unicode_script_round_trips() -> TestResult {
    let script = "a\0b\n\"quoted\" \u{1F5FA}\n".to_string();
    let mut input = sample_input();
    input.script = script.clone();
    let files = build_job_files(&input)?;
    assert_eq!(file(&files, "script.R")?.data, script.as_bytes());
    assert!(params_of(&files).is_ok());
    Ok(())
}
