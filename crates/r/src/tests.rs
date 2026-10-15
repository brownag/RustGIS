use super::*;
use std::cell::RefCell;

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
fn stub_run_reports_progress_once_and_is_marked_stub() -> TestResult {
    let updates = RefCell::new(Vec::new());
    let out = RscriptRunner.run(&sample_input(), &|u| updates.borrow_mut().push(u))?;
    assert_eq!(out.metadata["stub"], serde_json::Value::Bool(true));
    assert_eq!(updates.borrow().as_slice(), &[ExecutionUpdate::Progress(1.0)]);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stub_run_accepts_an_empty_script_without_panicking() -> TestResult {
    let input = RExecutionInput { script: String::new(), layers: Vec::new(), args: serde_json::Value::Null, ..Default::default() };
    let out = default_runner().run(&input, &|_| {})?;
    assert_eq!(out.metadata["stub"], serde_json::Value::Bool(true));
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
