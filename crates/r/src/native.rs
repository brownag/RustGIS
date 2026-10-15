//! The native runner: runs a script with the `Rscript` program in a fresh job folder, with a time
//! limit, and reads back what the script wrote to `out/`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::job::build_job_files;
use crate::wire::{MAX_FILE_BYTES, MAX_FILES, MAX_MESSAGES, clamp_timeout};
use crate::{ExecutionUpdate, RExecutionInput, RExecutionOutput, RFile, RRunnerError, RScriptRunner};

/// The file the program's output goes to, in the job folder.
const LOG_FILE: &str = "rscript.log";

/// The last bytes of the log kept as output (1 MiB), so a script that prints without end can't
/// fill memory.
const LOG_TAIL_BYTES: u64 = 1 << 20;

/// How often a running program is checked for an end or the time limit.
const POLL: Duration = Duration::from_millis(20);

/// Runs scripts with the `Rscript` program.
#[derive(Debug, Default, Clone, Copy)]
pub struct RscriptRunner;

/// A scratch folder for one run, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    /// A new, empty folder: never one that already exists (left over, or made by someone else
    /// on a shared machine), so no old file can pass for a script's output.
    fn new() -> Result<Scratch, String> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let mut last = None;
        for _ in 0..16 {
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("rustgis-r-{}-{nanos:x}-{n}", std::process::id()));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Scratch(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => return Err(format!("can't create a scratch folder: {e}")),
            }
        }
        Err(format!("can't create a scratch folder: {}", last.map(|e| e.to_string()).unwrap_or_default()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fail(e: impl std::fmt::Display) -> RRunnerError {
    RRunnerError::Failed(e.to_string())
}

impl RscriptRunner {
    /// The `Rscript` program that answers `--version` (see `discover::find`).
    pub fn locate(&self) -> Result<PathBuf, RRunnerError> {
        crate::discover::find().map(|(exe, _)| exe).ok_or(RRunnerError::NotFound)
    }
}

impl RScriptRunner for RscriptRunner {
    fn run(&self, input: &RExecutionInput, progress: &dyn Fn(ExecutionUpdate)) -> Result<RExecutionOutput, RRunnerError> {
        progress(ExecutionUpdate::Progress(0.0));
        let job = build_job_files(input).map_err(fail)?;
        let program = self.locate()?;
        let scratch = Scratch::new().map_err(fail)?;
        let dir = scratch.0.as_path();
        for file in &job {
            let path = dir.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(fail)?;
            }
            std::fs::write(&path, &file.data).map_err(fail)?;
        }
        std::fs::create_dir(dir.join("out")).map_err(fail)?;

        let limit = clamp_timeout(input.timeout_s);
        let log = File::create(dir.join(LOG_FILE)).map_err(fail)?;
        let log_err = log.try_clone().map_err(fail)?;
        let mut cmd = crate::discover::command(&program);
        cmd.args(["--no-save", "--no-restore", "--no-init-file", "bootstrap.R"])
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log_err)
            .env("LANGUAGE", crate::job::r_language(&input.lang));
        let mut child = cmd.spawn().map_err(fail)?;
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(fail)? {
                break status;
            }
            if start.elapsed() >= Duration::from_secs(limit) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RRunnerError::Failed(format!("the R script was stopped after {limit} s")));
            }
            std::thread::sleep(POLL);
        };

        let text = read_log_tail(&dir.join(LOG_FILE)).map_err(fail)?;
        let lines: Vec<&str> = text.lines().collect();
        if !status.success() {
            let last = lines.iter().rev().find(|l| !l.trim().is_empty()).map_or("no output", |l| l.trim());
            return Err(RRunnerError::Failed(format!("R exited with {status}: {last}")));
        }
        let stdout = lines.iter().rev().take(MAX_MESSAGES).rev().copied().collect::<Vec<_>>().join("\n");

        let files = collect_outputs(&dir.join("out")).map_err(RRunnerError::Failed)?;
        progress(ExecutionUpdate::Progress(1.0));
        Ok(RExecutionOutput { files, stdout, ..Default::default() })
    }
}

/// The last `LOG_TAIL_BYTES` of the log as text. A line cut off at the start is dropped.
fn read_log_tail(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let skip = file.metadata()?.len().saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(skip))?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if skip == 0 {
        return Ok(text);
    }
    Ok(text.split_once('\n').map_or(String::new(), |(_, rest)| rest.to_string()))
}

/// The plain files in `out`, read back as `out/<name>` and sorted by path. Folders and symlinks
/// are skipped, never followed or counted. More than `MAX_FILES` files, or a file over
/// `MAX_FILE_BYTES`, is an error.
pub(crate) fn collect_outputs(out: &Path) -> Result<Vec<RFile>, String> {
    let read_err = |e: std::io::Error| format!("can't read the script's output: {e}");
    let mut files = Vec::new();
    let mut total: u64 = 0;
    for entry in std::fs::read_dir(out).map_err(read_err)? {
        let entry = entry.map_err(read_err)?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(read_err)?;
        if !meta.file_type().is_file() {
            continue;
        }
        if files.len() >= MAX_FILES {
            return Err(format!("a script may write at most {MAX_FILES} files"));
        }
        if meta.len() > MAX_FILE_BYTES as u64 {
            return Err(format!("a script's output file is larger than the {MAX_FILE_BYTES}-byte limit"));
        }
        let mut data = Vec::new();
        File::open(&path).map_err(read_err)?.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut data).map_err(read_err)?;
        if data.len() > MAX_FILE_BYTES {
            return Err(format!("a script's output file is larger than the {MAX_FILE_BYTES}-byte limit"));
        }
        total = total.saturating_add(data.len() as u64);
        if total > MAX_FILES as u64 * MAX_FILE_BYTES as u64 {
            return Err("a script's output is larger than the total limit".into());
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        files.push(RFile { path: format!("out/{name}"), data });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
