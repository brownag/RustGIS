//! The R queue of `gp.runR`: staging a run's inputs, the queue of runs and their results. The
//! shared queue (`started`, trimming) and the finishing step stay in `gp.rs`.

use rustgis_analysis::Inputs;
use serde_json::{Value, json};

use super::gp::{ExternalJob, MAX_ASYNC_RUNS, map_inputs, now_ms, trim_started};
use super::ps;
use crate::{CmdResult, CommandSpec as C, Session};

pub fn commands() -> Vec<C> {
    vec![
        C::new("gp.runR", "Run R Script", "Analyze › Geoprocessing", run_r).params(
            r#"{"script": the R code, "inputs"?: [layer names], "args"?: {...} (values for the script), "timeout_s"?: seconds, "lang"?: "the interface language code (empty: English)"}"#,
        ),
        C::new("gp.setRscript", "Set Rscript", "Analyze › Geoprocessing", set_rscript)
            .view()
            .params(r#"{"path"?: "the Rscript program (empty: look in the usual places)"}"#),
    ]
}

/// The browser's R job, for the `Services::r_run` hook (see [`r_start`]). The web app leaves that
/// hook `None` until webR (P6) is wired in.
pub struct RRun {
    pub input: rustgis_r::RExecutionInput,
}

/// The text a runner failure starts with (no Rscript, a crash, the time limit). A script's own
/// error comes without it.
pub const R_FAILURE_PREFIX: &str = "R script: ";

/// The text for a run that was dropped before it finished.
const DROPPED_R_RUN: &str = "the R script's run was dropped (its project was closed, or too many runs were waiting)";

/// The checked parameters of a `gp.runR` run (see [`r_params`]).
pub struct RParams {
    pub script: String,
    pub inputs: Vec<String>,
    pub args: Value,
    /// The time limit in seconds (clamped), or `None` for the default.
    pub timeout_s: Option<u64>,
    /// The interface language code (such as `de`); empty means English.
    pub lang: String,
}

/// Checks `gp.runR` parameters: a non-empty `script` (at most 1 MB), `inputs` as a list of layer
/// names, and `args` as an object. It stages and queues nothing.
pub fn r_params(params: &Value) -> Result<RParams, String> {
    let script = ps(params, "script").map(str::trim).filter(|x| !x.is_empty()).ok_or("the R script is required")?;
    if script.len() > rustgis_r::MAX_SCRIPT_BYTES {
        return Err("the R script is over 1 MB".into());
    }
    let names = match params.get("inputs") {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(a)) => a.as_slice(),
        Some(_) => return Err("inputs must be a list of layer names".into()),
    };
    let mut inputs = Vec::with_capacity(names.len());
    for name in names {
        inputs.push(name.as_str().ok_or("inputs must be a list of layer names")?.to_string());
    }
    let args = match params.get("args") {
        None | Some(Value::Null) => json!({}),
        Some(v @ Value::Object(_)) => v.clone(),
        Some(_) => return Err("args must be an object".into()),
    };
    let timeout_s = match params.get("timeout_s") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let n = v.as_i64().ok_or("timeout_s must be a whole number of seconds")?;
            Some(rustgis_r::clamp_timeout(Some(n.max(0) as u64)))
        }
    };
    let lang = ps(params, "lang").unwrap_or("").to_string();
    Ok(RParams { script: script.to_string(), inputs, args, timeout_s, lang })
}

/// Stages a `gp.runR` run's inputs from the map (read only) and queues it. Returns the run when
/// no R run is under way, so the host launches it; otherwise the run waits its turn and this
/// returns `None`.
pub fn r_start(s: &mut Session, params: &Value) -> Result<Option<RRunStart>, String> {
    let p = r_params(params)?;
    let (layers, rows) = {
        let inputs = map_inputs(s, None, &json!(p.inputs))?;
        let mut layers = Vec::with_capacity(p.inputs.len());
        let mut rows = Vec::new();
        for name in &p.inputs {
            let layer = s.map().layer_by_name(name).ok_or_else(|| format!("no layer named '{name}'"))?;
            if layer.is_raster() {
                let raster = inputs.raster(name).ok_or_else(|| format!("layer '{name}' has no raster data"))?;
                layers.push(rustgis_r::stage::stage_raster(name, raster.embedded, raster.raster)?.layer);
            } else {
                let ds = inputs.dataset(name).ok_or_else(|| format!("no layer named '{name}'"))?;
                let staged = rustgis_r::stage::stage_dataset(ds, inputs.selection(name))?;
                rows.push((staged.layer.name.clone(), staged.read_rows));
                layers.push(staged.layer);
            }
        }
        (layers, rows)
    };
    let queued = s.started.iter().filter(|(_, job, ..)| matches!(job, ExternalJob::R(_))).count();
    if queued >= MAX_ASYNC_RUNS {
        return Err("too many R scripts are waiting".into());
    }
    s.next_pending = s.next_pending.wrapping_add(1);
    let id = s.next_pending;
    let input = rustgis_r::RExecutionInput { script: p.script, layers, args: p.args, timeout_s: p.timeout_s, lang: p.lang };
    let start = if s.r_running.is_none() {
        s.r_running = Some(id);
        Some(RRunStart { id, input: input.clone() })
    } else {
        None
    };
    s.started.push((id, ExternalJob::R(RJob { input, rows }), params.clone(), now_ms()));
    trim_started(s);
    Ok(start)
}

/// The text of a failed R run and its typed notice, if it has one.
fn r_failure(e: rustgis_r::RRunnerError) -> (String, Option<rustgis_r::RNotice>) {
    use rustgis_r::{RNotice, RRunnerError};
    match e {
        RRunnerError::Notice(n) => (n.english().into(), Some(n)),
        RRunnerError::NotFound => (RNotice::NotFound.english().into(), Some(RNotice::NotFound)),
        RRunnerError::Unsupported(m) => (format!("not supported here: {m}"), None),
        RRunnerError::Failed(m) => (m, None),
    }
}

/// Finishes an R run from [`r_start`]: its output becomes a result that `gp.finish` applies. Then
/// it hands back the next queued R run for the host to launch, if none is under way.
pub fn r_complete(
    s: &mut Session,
    id: u64,
    ran: Result<rustgis_r::RExecutionOutput, rustgis_r::RRunnerError>,
) -> Result<Option<RRunStart>, String> {
    let i = s.started.iter().position(|(n, j, ..)| *n == id && matches!(j, ExternalJob::R(_))).ok_or(DROPPED_R_RUN)?;
    let (_, job, params, t0) = s.started.remove(i);
    #[cfg(feature = "geolibre")]
    let ExternalJob::R(job) = job else {
        return Err(DROPPED_R_RUN.into());
    };
    #[cfg(not(feature = "geolibre"))]
    let ExternalJob::R(job) = job;
    if s.r_running == Some(id) {
        s.r_running = None;
    }
    // Reading the host's output parses outside `Session::run`'s guard: a bug in a reader must not
    // take the app down.
    let rows = job.rows;
    let rows_of = |name: &str| {
        let exact = rows.iter().find(|(n, _)| n.as_str() == name);
        exact.or_else(|| rows.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))).map(|(_, r)| r.clone())
    };
    // convert_output catches panics in the reader itself, so no guard is needed here.
    let (result, r_notice) = match ran {
        Err(e) => {
            let (text, notice) = r_failure(e);
            (Err(format!("{R_FAILURE_PREFIX}{text}")), notice)
        }
        Ok(out) => match rustgis_r::manifest::convert_output(&out, &rows_of) {
            Ok(result) => (Ok(result), None),
            Err(e) => {
                let (text, notice) = r_failure(e);
                (Err(text), notice)
            }
        },
    };
    s.pending.push(crate::PendingRun {
        id,
        tool: "R script".into(),
        // No tool id: an R run isn't run again from the history.
        tool_id: String::new(),
        params,
        result,
        started: t0,
        millis: now_ms().saturating_sub(t0),
        r_notice,
    });
    if s.pending.len() > MAX_ASYNC_RUNS {
        s.pending.remove(0);
    }
    if s.r_running.is_some() {
        return Ok(None);
    }
    #[cfg(feature = "geolibre")]
    let next = s.started.iter().find_map(|(n, j, ..)| match j {
        ExternalJob::R(job) => Some((*n, job.input.clone())),
        ExternalJob::GeoLibre(_) => None,
    });
    #[cfg(not(feature = "geolibre"))]
    let next = s.started.first().map(|(n, j, ..)| match j {
        ExternalJob::R(job) => (*n, job.input.clone()),
    });
    match next {
        Some((next_id, input)) => {
            s.r_running = Some(next_id);
            Ok(Some(RRunStart { id: next_id, input }))
        }
        None => Ok(None),
    }
}

/// Queues an R script for the host to run (the desktop runs it on a thread, the CLI on its own
/// thread; see [`r_start`]). The parameters and input names are checked first. The engine never
/// runs R itself. The browser refuses until its runner is wired in.
fn run_r(s: &mut Session, p: &Value) -> CmdResult {
    let params = r_params(p)?;
    for name in &params.inputs {
        s.map().layer_by_name(name).ok_or_else(|| format!("no layer named '{name}'"))?;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // The host launches the run when `r_start` hands it back; the id is the run just queued.
        r_start(s, p)?;
        Ok(json!({"queued": s.next_pending}))
    }
    #[cfg(target_arch = "wasm32")]
    Err("R scripts can't run in this build yet".into())
}

/// The state of the Rscript choice: whether a program was found, its path and version. Native only.
#[cfg(all(feature = "r", not(target_arch = "wasm32")))]
fn rscript_status() -> Value {
    let found = rustgis_r::find();
    json!({
        "supported": true,
        "found": found.is_some(),
        "path": found.as_ref().map(|(p, _)| p.display().to_string()),
        "version": found.map(|(_, v)| v),
    })
}

/// Chooses the Rscript program (an empty path looks in the usual places again). Native only.
fn set_rscript(_s: &mut Session, p: &Value) -> CmdResult {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = ps(p, "path").map(str::trim).filter(|p| !p.is_empty()).map(std::path::PathBuf::from);
        rustgis_r::choose(path)?;
        Ok(rscript_status())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = p;
        Err("this build has no Rscript to choose".into())
    }
}

/// A staged R run waiting to start: its input, and the host rows each vector layer was read from
/// at staging (the rows the script's results refer to).
pub(crate) struct RJob {
    pub(crate) input: rustgis_r::RExecutionInput,
    pub(crate) rows: Vec<(String, Vec<usize>)>,
}

/// An R run the host launches now (see [`r_start`]). The engine never runs R itself: the host
/// runs `input` and reports the result with `id`.
#[derive(Clone, Debug)]
pub struct RRunStart {
    pub id: u64,
    pub input: rustgis_r::RExecutionInput,
}
