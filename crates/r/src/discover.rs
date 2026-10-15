//! Finding the `Rscript` program: the one the user chose, then `RUSTGIS_RSCRIPT`, `R_HOME`, the
//! PATH and the usual install folders. A candidate is used only if it answers `--version`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The environment variable that names the program, ahead of the PATH.
const ENV: &str = "RUSTGIS_RSCRIPT";

/// A program the user chose (Locate Rscript…), ahead of the usual places.
static CHOSEN: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The program found, with its version, cached until the choice changes.
static FOUND: Mutex<Option<Option<(PathBuf, String)>>> = Mutex::new(None);

fn program_name() -> &'static str {
    if cfg!(windows) { "Rscript.exe" } else { "Rscript" }
}

/// A command for the program, without a console window on Windows.
fn command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Runs `c` to completion, or kills it after `limit`. A program that only happens to be called
/// `Rscript` mustn't hang the app.
fn output_within(mut c: Command, limit: Duration) -> Option<Output> {
    let mut child = c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    // Drain both pipes on threads while the process runs: a full pipe would otherwise block it
    // until the time limit.
    fn drain(mut pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    }
    let (Some(out), Some(err)) = (child.stdout.take(), child.stderr.take()) else {
        // Not expected with piped output, but never leave the process running.
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (out_reader, err_reader) = (drain(out), drain(err));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let stdout = out_reader.join().ok()?;
    let stderr = err_reader.join().unwrap_or_default();
    Some(Output { status, stdout, stderr })
}

/// The `Rscript (R) version …` line of some `--version` output, if there is one.
pub(crate) fn version_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| l.starts_with("Rscript (R) version")).map(str::to_string)
}

/// The version of `exe`, if it is an `Rscript` program. A missing file is `None` without a spawn.
pub fn version_of(exe: &Path) -> Option<String> {
    if !exe.is_file() {
        return None;
    }
    let mut c = command(exe);
    c.arg("--version");
    let out = output_within(c, Duration::from_secs(10))?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    version_line(&stdout).or_else(|| version_line(&stderr))
}

/// The sort key of an install folder name such as `R-4.10.0`: its numeric parts.
#[cfg(any(windows, test))]
pub(crate) fn version_key(name: &str) -> Vec<u64> {
    name.trim_start_matches("R-").split('.').map(|part| part.parse().unwrap_or(0)).collect()
}

/// The R install folders to look in, newest R first.
#[cfg(windows)]
fn install_dirs() -> Vec<PathBuf> {
    let Some(pf) = std::env::var_os("ProgramFiles") else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(PathBuf::from(pf).join("R")) else {
        return Vec::new();
    };
    let mut versions: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("R-")))
        .collect();
    versions.sort_by_key(|p| std::cmp::Reverse(version_key(p.file_name().and_then(|n| n.to_str()).unwrap_or_default())));
    versions.into_iter().flat_map(|v| [v.join("bin").join("x64"), v.join("bin")]).collect()
}

/// The folders where the program is usually installed.
#[cfg(not(windows))]
fn install_dirs() -> Vec<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/opt/homebrew/bin", "/Library/Frameworks/R.framework/Resources/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// The places the program is looked for, in order.
fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(c) = CHOSEN.lock()
        && let Some(p) = c.clone()
    {
        v.push(p);
    }
    if let Some(p) = std::env::var_os(ENV).filter(|p| !p.is_empty()) {
        v.push(PathBuf::from(p));
    }
    if let Some(home) = std::env::var_os("R_HOME").filter(|h| !h.is_empty()) {
        v.push(PathBuf::from(home).join("bin").join(program_name()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        // Absolute entries only: an empty or relative one would mean the current folder.
        v.extend(std::env::split_paths(&path).filter(|d| d.is_absolute()).map(|d| d.join(program_name())));
    }
    v.extend(install_dirs().into_iter().map(|d| d.join(program_name())));
    v
}

/// The program to use and its version, or `None` when there isn't one. The answer is kept until
/// the choice changes. Probing runs without holding the cache's lock.
pub fn find() -> Option<(PathBuf, String)> {
    if let Ok(found) = FOUND.lock()
        && let Some(f) = found.as_ref()
    {
        return f.clone();
    }
    let probed = candidates().into_iter().find_map(|p| version_of(&p).map(|v| (p, v)));
    if let Ok(mut found) = FOUND.lock() {
        // Another caller may have stored an answer meanwhile: the first one stays.
        return found.get_or_insert(probed).clone();
    }
    probed
}

/// Uses `exe` as the program (`None`: look in the usual places again). Fails, keeping the
/// current choice, when `exe` isn't an Rscript program.
pub fn choose(exe: Option<PathBuf>) -> Result<Option<(PathBuf, String)>, String> {
    if let Some(p) = &exe
        && version_of(p).is_none()
    {
        return Err(format!("{} is not an Rscript program", p.display()));
    }
    if let Ok(mut c) = CHOSEN.lock() {
        *c = exe;
    }
    if let Ok(mut f) = FOUND.lock() {
        *f = None;
    }
    Ok(find())
}

/// Checks that the program runs and that `jsonlite` and `sf` load. Returns the program.
pub fn ready() -> Result<PathBuf, String> {
    let (exe, _) = find().ok_or_else(|| crate::notices::NOT_FOUND.to_string())?;
    let script = "for (p in c(\"jsonlite\", \"sf\")) cat(p, \"=\", requireNamespace(p, quietly = TRUE), \"\\n\", sep = \"\")";
    let mut c = command(&exe);
    c.args(["--no-save", "--no-restore", "--no-init-file", "-e", script]);
    let out = output_within(c, Duration::from_secs(60)).ok_or_else(|| format!("{}: no answer from the R check", exe.display()))?;
    if !out.status.success() {
        return Err(format!("{}: the R check failed", exe.display()));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for pkg in ["jsonlite", "sf"] {
        if !text.lines().any(|l| l.trim() == format!("{pkg}=TRUE")) {
            return Err(format!("the R package {pkg} is not installed: run install.packages(\"{pkg}\") in R"));
        }
    }
    Ok(exe)
}
