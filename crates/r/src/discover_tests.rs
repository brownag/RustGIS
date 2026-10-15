use crate::discover::{choose, find, ready, version_key, version_line, version_of};
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn version_of_a_missing_path_is_none() {
    assert_eq!(version_of(Path::new("/nonexistent/rustgis/Rscript")), None);
}

#[test]
fn choose_refuses_a_file_that_is_not_rscript() -> TestResult {
    let path = std::env::temp_dir().join(format!("rustgis-r-discover-{}-refuse", std::process::id()));
    std::fs::write(&path, "not an R program\n")?;
    let result = choose(Some(path.clone()));
    let _ = std::fs::remove_file(&path);
    let err = result.err().ok_or("choose accepted a text file")?;
    assert!(err.contains("is not an Rscript program"), "{err}");
    Ok(())
}

#[test]
fn version_line_accepts_the_banner_and_rejects_other_text() {
    assert_eq!(version_line("Rscript (R) version 4.6.1 (2026-06-24)\n").as_deref(), Some("Rscript (R) version 4.6.1 (2026-06-24)"));
    assert_eq!(version_line("R version 4.6.1"), None);
    assert_eq!(version_line("hello\nworld"), None);
}

#[test]
fn version_key_sorts_numeric_parts() {
    assert!(version_key("R-4.10.0") > version_key("R-4.9.2"));
}

#[test]
fn find_returns_a_program_that_passes_version_of() {
    let Some((exe, version)) = find() else {
        println!("skipped: Rscript not available");
        return;
    };
    assert_eq!(version_of(&exe), Some(version));
}

#[test]
fn ready_is_ok_or_names_the_missing_package() {
    if find().is_none() {
        println!("skipped: Rscript not available");
        return;
    }
    match ready() {
        Ok(exe) => assert!(version_of(&exe).is_some()),
        Err(msg) => assert!(msg.contains("install.packages("), "{msg}"),
    }
}
