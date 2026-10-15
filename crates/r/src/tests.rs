use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn sample_input() -> RExecutionInput {
    RExecutionInput {
        script: "1 + 1".into(),
        layers: vec![RLayerInput { name: "Cities".into(), format: "geojson".into(), data: b"{}".to_vec(), ..Default::default() }],
        args: serde_json::json!({ "k": 3 }),
        ..Default::default()
    }
}

#[test]
fn input_round_trips_through_json() -> TestResult {
    let input = sample_input();
    let back: RExecutionInput = serde_json::from_str(&serde_json::to_string(&input)?)?;
    assert_eq!(back, input);
    Ok(())
}

#[test]
fn output_round_trips_through_json() -> TestResult {
    let out = RExecutionOutput {
        tables: vec![serde_json::json!({ "x": [1, 2] })],
        stdout: "hi\n".into(),
        metadata: serde_json::json!({ "rows": 2 }),
        ..Default::default()
    };
    let back: RExecutionOutput = serde_json::from_str(&serde_json::to_string(&out)?)?;
    assert_eq!(back, out);
    Ok(())
}

#[test]
fn updates_round_trip_through_json() -> TestResult {
    let updates = [ExecutionUpdate::Progress(0.5), ExecutionUpdate::Log { level: LogLevel::Warn, message: "careful".into() }];
    for update in updates {
        let back: ExecutionUpdate = serde_json::from_str(&serde_json::to_string(&update)?)?;
        assert_eq!(back, update);
    }
    Ok(())
}

#[test]
fn hostile_json_is_an_error_not_a_panic() {
    assert!(serde_json::from_str::<ExecutionUpdate>(r#"{"Progress":"x"}"#).is_err());
    assert!(serde_json::from_str::<ExecutionUpdate>(r#"{"Log":{"level":"trace","message":"m"}}"#).is_err());
    assert!(serde_json::from_str::<RExecutionInput>("not json").is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn run_without_r_reports_the_missing_program_or_runs() -> TestResult {
    // The sample input's geojson layer is refused before the program is looked for, so use no layers.
    let input = RExecutionInput { script: "1 + 1".into(), ..Default::default() };
    let _r = crate::harness_tests::lock_r();
    let result = RscriptRunner.run(&input, &|_| {});
    if RscriptRunner.locate().is_err() {
        println!("skipped: Rscript not available");
        assert_eq!(result, Err(RRunnerError::NotFound));
    } else {
        assert!(result.is_ok(), "{result:?}");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn run_refuses_an_empty_script() -> TestResult {
    let input = RExecutionInput { script: String::new(), layers: Vec::new(), args: serde_json::Value::Null, ..Default::default() };
    let Err(err) = default_runner().run(&input, &|_| {}) else {
        return Err("expected the empty script to be refused".into());
    };
    assert!(err.to_string().contains("the script is empty"), "{err}");
    Ok(())
}

/// A folder under the temp dir with an `out/` inside, removed on drop.
#[cfg(not(target_arch = "wasm32"))]
struct CapsDir(std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl CapsDir {
    fn new(test: &str) -> std::io::Result<Self> {
        let base = std::env::temp_dir().join(format!("rustgis-r-caps-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("out"))?;
        Ok(Self(base))
    }

    fn out(&self) -> std::path::PathBuf {
        self.0.join("out")
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for CapsDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn collect_outputs_refuses_too_many_and_oversize_files() -> TestResult {
    let dir = CapsDir::new("limits")?;
    for i in 0..=MAX_FILES {
        std::fs::write(dir.out().join(format!("f{i}.txt")), b"x")?;
    }
    let Err(err) = crate::native::collect_outputs(&dir.out()) else {
        return Err("expected too many files to be refused".into());
    };
    assert!(err.contains("at most 256 files"), "{err}");

    let big = CapsDir::new("oversize")?;
    std::fs::File::create(big.out().join("big.bin"))?.set_len(MAX_FILE_BYTES as u64 + 1)?;
    let Err(err) = crate::native::collect_outputs(&big.out()) else {
        return Err("expected the oversize file to be refused".into());
    };
    assert!(err.contains("limit"), "{err}");
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), unix))]
#[test]
fn collect_outputs_skips_folders_and_symlinks() -> TestResult {
    let dir = CapsDir::new("links")?;
    let secret = dir.0.join("secret.txt");
    std::fs::write(&secret, b"secret")?;
    std::fs::create_dir(dir.out().join("sub"))?;
    std::os::unix::fs::symlink(&secret, dir.out().join("link"))?;
    std::fs::write(dir.out().join("kept.txt"), b"ok")?;
    let files = crate::native::collect_outputs(&dir.out())?;
    assert_eq!(files, vec![RFile { path: "out/kept.txt".into(), data: b"ok".to_vec() }]);
    Ok(())
}

#[test]
fn old_format_json_still_parses() -> TestResult {
    let layer: RLayerInput = serde_json::from_str(r#"{"name":"a","format":"fgb","data":[1,2]}"#)?;
    assert_eq!(layer.kind, RLayerKind::Vector);
    assert!(layer.selection.is_empty());
    let input: RExecutionInput = serde_json::from_str(r#"{"script":"1","layers":[],"args":{}}"#)?;
    assert_eq!(input.timeout_s, None);
    assert_eq!(input.lang, "");
    let out: RExecutionOutput = serde_json::from_str(r#"{"tables":[],"stdout":"","metadata":null}"#)?;
    assert!(out.files.is_empty());
    Ok(())
}

#[test]
fn new_fields_round_trip() -> TestResult {
    let input = RExecutionInput {
        script: "1".into(),
        layers: vec![RLayerInput {
            name: "t".into(),
            format: "csv".into(),
            data: b"a\n1".to_vec(),
            kind: RLayerKind::Table,
            selection: vec![1, 5],
        }],
        args: serde_json::json!({}),
        timeout_s: Some(30),
        lang: "fr".into(),
    };
    let back: RExecutionInput = serde_json::from_str(&serde_json::to_string(&input)?)?;
    assert_eq!(back, input);
    let out = RExecutionOutput { files: vec![RFile { path: "out/x.csv".into(), data: b"x".to_vec() }], ..Default::default() };
    let back: RExecutionOutput = serde_json::from_str(&serde_json::to_string(&out)?)?;
    assert_eq!(back, out);
    Ok(())
}

#[test]
fn clamp_timeout_keeps_the_limit_in_range() {
    assert_eq!(clamp_timeout(None), 120);
    assert_eq!(clamp_timeout(Some(0)), 1);
    assert_eq!(clamp_timeout(Some(1_000_000_000)), 86_400);
    assert_eq!(clamp_timeout(Some(30)), 30);
}

#[test]
fn clean_lang_keeps_only_plain_tags() {
    assert_eq!(clean_lang("fr"), "fr");
    assert_eq!(clean_lang("zh-CN"), "zh-CN");
    assert_eq!(clean_lang(""), "en");
    assert_eq!(clean_lang("a b"), "en");
    assert_eq!(clean_lang(&"a".repeat(40)), "en");
    assert_eq!(clean_lang("é"), "en");
}

#[test]
fn layer_kind_uses_lowercase_names() -> TestResult {
    assert_eq!(serde_json::to_string(&RLayerKind::Vector)?, r#""vector""#);
    assert_eq!(serde_json::to_string(&RLayerKind::Table)?, r#""table""#);
    assert_eq!(serde_json::to_string(&RLayerKind::Raster)?, r#""raster""#);
    Ok(())
}
