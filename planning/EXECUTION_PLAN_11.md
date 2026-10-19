# EXECUTION PLAN 11 -- resolve the R core review findings

Branch: `feature/engine-r-core-main` (base for the review: `origin/main`).
Source of findings: the Standards and Spec reviews of `git diff origin/main...HEAD` (2026-10-18).
Verification gate for every task: targeted tests named in the spec. Before the final wave:
`cargo xtask ci` and `cargo build --target wasm32-unknown-unknown -p rustgis-web`.

## Goal & Scope

**Goal.** Close the findings from the R core review: desktop R runs must not block the UI thread,
R failure paths must produce useful notices and manifests, the output size limit must be a real
bound, and the dead or duplicated code and stale docs the review named must go.

**In scope**
- Spec 1: desktop `gp.runR` runs on a background thread through the existing R queue (RFC line 46, D16).
- Spec 2: the missing-Rscript notice gives install instructions (RFC, D25).
- Spec 3: a failure manifest is written when setup fails, not only when the script fails (P1).
- Spec C1: a total output size bound, checked before reading (D22).
- Spec C2: an `ok:true` manifest with no layers and no selection is a valid result (D4).
- Standards: stale doc references, dead code, duplicated R-run checks, duplicated notice and error
  paths, the `let v` binding, and the duplicated "run was dropped" text.

**Out of scope (deferred, do not start)**
- Cancel of a running R job (D8, P4).
- The R Script pane, live log and Running status (P4).
- The `r:` tool catalog (P5) and webR in the browser (P6). The web `r_run` hook stays empty.
- The 15 `.tsv` translations. Missing entries show English; the maintainer translates before release.
- Streaming output (D26).

**Verbatim constraints (from the review and AGENTS.md)**
- "Never crash": no `unwrap`/`expect`/`panic`/`unreachable`/`todo` in non-test code; no `unsafe`.
- "Every crash fix gets a regression test."
- "Everything below `apps/` must build for `wasm32-unknown-unknown`; use `web_time`."
- "Commands never open dialogs." UI literals use `tl!`; messages use `i18n::fmt`.

## Decision log (2026-10-18)

Recommended defaults, taken without a one-at-a-time interview. Each is a judgement call for approval.

- **D1** Spec 1 is in scope now. Cancel stays deferred to P4. The desktop reuses the R queue
  (`r_start`/`r_complete`) and the `external_start`-style hand-off that the geolibre desktop path uses.
- **D2** The `Services::r_run` hook stays for webR (P6). Only its doc comment changes.
- **D3** The `r:` tool prefix guard stays (it protects tools not yet written). Its call sites are
  consolidated into one predicate.
- **D4** A script may end with `ok:true` and neither layers nor a selection (it only prints).
- **D5** The total output bound is `MAX_TOTAL_BYTES` = 512 MiB, summed from file metadata before any read.
- **D6** The missing-Rscript notice text gains the install and `$RUSTGIS_RSCRIPT_PATH` instructions.
- **D7** The bootstrap's top-level failure handler covers params parsing and `out/` setup.
- **D8** `discover::ready()` is deleted. Its tests call `find()` instead.

## Wave table

| Wave | Task | Repo | Must edit (files) | Depends on |
|---|---|---|---|---|
| 1 | 11.1.1 output bounds (C1) | rustgis | `crates/r/src/native.rs`, `crates/r/src/tests.rs` | -- |
| 1 | 11.1.2 empty manifest is valid (C2) | rustgis | `crates/r/src/manifest.rs`, `crates/r/src/manifest_tests.rs` | -- |
| 1 | 11.1.3 bootstrap setup failures (Spec 3) | rustgis | `crates/r/src/bootstrap.R`, `crates/r/src/harness_tests.rs` | -- |
| 1 | 11.1.4 Rscript-not-found instructions (Spec 2) | rustgis | `crates/r/src/notices.rs`, `crates/ui-egui/src/rnotices.rs`, `crates/r/src/notices_tests.rs` (new) | -- |
| 1 | 11.1.5 delete `ready()` (dead code) | rustgis | `crates/r/src/discover.rs`, `crates/r/src/discover_tests.rs` | -- |
| 1 | 11.1.6 doc drift (stale web runner text) | rustgis | `crates/r/src/lib.rs` (doc only), `crates/engine/src/cmd/gp_r.rs` (doc only), `crates/ui-egui/src/lib.rs` (doc only) | -- |
| 1 | 11.1.7 inline `let v` in `gp.rs` | rustgis | `crates/engine/src/cmd/gp.rs` | -- |
| 2 | 11.2.1 desktop R runs in the background (Spec 1) | rustgis | `crates/engine/src/cmd/gp_r.rs`, `crates/ui-egui/src/lib.rs` (hook wiring only) | 11.1.6, 11.1.7 |
| 2 | 11.2.2 one R-run predicate (Standards) | rustgis | `crates/engine/src/cmd/plugins.rs`, `crates/engine/src/cmd/workflows.rs`, `crates/engine/src/cmd/items.rs`, `crates/assistant/src/tools.rs`, `crates/assistant/src/mcp.rs` | 11.1.5 |
| 3 | 11.3.1 one notice and error path (Standards) | rustgis | `crates/r/src/lib.rs`, `crates/r/src/notices.rs`, `crates/ui-egui/src/rnotices.rs`, `crates/engine/src/cmd/gp_r.rs`, `crates/engine/src/cmd/gp.rs` | 11.2.1, 11.1.4 |

**Disjointness check.** Within each wave the file sets do not overlap. Across waves, every shared
file (`gp_r.rs`, `gp.rs`, `ui-egui/src/lib.rs`, `r/src/lib.rs`, `notices.rs`, `rnotices.rs`) is
touched in an earlier wave, so the later wave's worker reads the earlier result.

**Worktrees.** Wave 1 has seven tasks in one repo. Use worktree isolation, and create the
worktrees by hand from `feature/engine-r-core-main`, not `main` (the auto-worktree base is `main`).

---

## Task specs

### 11.1.1 output bounds (C1)

- **Files in scope.** Must edit: `crates/r/src/native.rs`, `crates/r/src/tests.rs`.
- **Spec.** `collect_outputs` (`native.rs` ~line 139) reads each file into memory, and its total
  check (~line 162) is `MAX_FILES * MAX_FILE_BYTES` (128 GiB). Add `const MAX_TOTAL_BYTES: u64 =
  512 * 1024 * 1024` next to `MAX_FILE_BYTES`. Sum `meta.len()` for every plain file first, and
  return `"a script's output is larger than the total limit"` before any file is read. Keep the
  per-file read check as it is.
- **Anchors.** `fn collect_outputs`, `MAX_FILE_BYTES`, `MAX_FILES`. Existing tests:
  `tests.rs` `collect_outputs_refuses_too_many_and_oversize_files` (~line 106). Reference: that
  test's helper style.
- **Constraints.** Do not touch the manifest code. Keep symlinks skipped.
- **Acceptance.** `cargo test -p rustgis-r collect_outputs`. Add one regression test: files whose
  sizes sum past `MAX_TOTAL_BYTES` are refused with the total-limit error. Use sparse files
  (`File::set_len`) so the test stays fast and writes no real data.

### 11.1.2 empty manifest is valid (C2)

- **Files in scope.** Must edit: `crates/r/src/manifest.rs`, `crates/r/src/manifest_tests.rs`.
- **Spec.** `manifest.rs` ~line 57 rejects `ok:true` when neither layers nor a selection is given.
  Per D4, accept it: an empty output with the messages. Keep the rejection when both are given.
  Update the message wording if it still says "not neither".
- **Anchors.** `layers.is_empty() == selection.is_none()`; `convert_messages`.
- **Constraints.** Hostile input: a `layers` value that is not a list stays an error.
- **Acceptance.** `cargo test -p rustgis-r manifest`. Tests: `ok:true` with no layers and no
  selection gives an empty output with its messages; `ok:true` with both still errors.

### 11.1.3 bootstrap setup failures (Spec 3)

- **Files in scope.** Must edit: `crates/r/src/bootstrap.R`, `crates/r/src/harness_tests.rs`.
- **Spec.** Per D7, wrap the setup in `.rgis_main` (params read, `out/` creation) in a `tryCatch`.
  On error, write `out/manifest.json` with `ok:false` and the message, the way the jsonlite
  branch already does by hand. Keep `.rgis_run` for the script itself. Mirror the header comment
  (lines 16–18), which must say the failure manifest now covers setup too.
- **Anchors.** `.rgis_main`; `.rgis_run`; the hand-written manifest branch for missing jsonlite
  (`bootstrap.R` ~line 232). Harness tests run the bootstrap under Rscript when R is present.
- **Constraints.** Escape `"` and `\` in the message the same way the jsonlite branch does.
  If R is absent the harness test skips; say so in the report.
- **Acceptance.** `cargo test -p rustgis-r harness`. Add a test: a `params.json` with version 2
  gives `ok:false` in `out/manifest.json` (skip when R is absent).

### 11.1.4 Rscript-not-found instructions (Spec 2)

- **Files in scope.** Must edit: `crates/r/src/notices.rs`, `crates/ui-egui/src/rnotices.rs`.
  Can create: `crates/r/src/notices_tests.rs` (wire it with the same test-module convention as
  the other `*_tests.rs` files in `crates/r/src`).
- **Spec.** Per D6, change `NOT_FOUND` (`notices.rs` line 6) to say how to fix it: install R
  (`https://cran.r-project.org`) or set `RUSTGIS_RSCRIPT_PATH`, and that the Rscript choice can
  be set with Locate Rscript. Keep the code `not_found` stable. Update `rnotices.rs` so the
  translated text is the new English. Do not touch the `.tsv` catalogs.
- **Anchors.** `pub const NOT_FOUND`; `RNotice::NotFound`; `rnotices.rs` `fn text`.
- **Constraints.** Keep the text in step with `bootstrap.R` if it carries the same string.
- **Acceptance.** `cargo test -p rustgis-r notices`. The test asserts the notice names
  `RUSTGIS_RSCRIPT_PATH`. `cargo test -p rustgis-ui-egui rnotices` if a test exists.

### 11.1.5 delete `ready()` (dead code)

- **Files in scope.** Must edit: `crates/r/src/discover.rs`, `crates/r/src/discover_tests.rs`.
- **Spec.** Per D8, delete `pub fn ready()` (`discover.rs` line 181) and its doc. The tests
  `ready_is_ok_or_names_the_missing_package` and `ready_is_ok_when_r_is_present` rewrite to call
  `find()`. Keep the assertions' intent.
- **Anchors.** `discover_tests.rs` lines 44–66.
- **Constraints.** Check the crate root `lib.rs` for a re-export of `ready` and remove it only if
  it is in your scope. If `lib.rs` re-exports it, STOP and report: `lib.rs` is owned by 11.1.6.
- **Acceptance.** `cargo test -p rustgis-r discover`. Also `cargo check -p rustgis-r`.

### 11.1.6 doc drift (stale web runner text)

- **Files in scope.** Must edit: `crates/r/src/lib.rs`, `crates/engine/src/cmd/gp_r.rs`,
  `crates/ui-egui/src/lib.rs`. Doc comments only; no code.
- **Spec.** Three stale texts:
  - `crates/r/src/lib.rs:3` says the browser runner lives in `web.rs`. That file was removed.
    Point the doc at `Services::r_run` in `crates/ui-egui/src/lib.rs`, or delete the sentence.
  - `crates/engine/src/cmd/gp_r.rs:24` (`RRun` doc) and `crates/ui-egui/src/lib.rs` ~line 196
    (`r_run` doc) call the web runner a "placeholder". It is an empty hook: the web app sets
    `r_run: None` (`apps/rustgis-web/src/web.rs` ~line 671). Say that.
  - Keep D2: the hook is for webR in P6.
- **Anchors.** The grep strings `web.rs`, `placeholder`.
- **Constraints.** Doc text only. Do not change any function signature.
- **Acceptance.** `cargo check -p rustgis-r -p rustgis-engine -p rustgis-ui-egui`. The
  `grep -rn "placeholder" crates/engine/src/cmd/gp_r.rs crates/ui-egui/src/lib.rs` shows no R-hook hit.

### 11.1.7 inline `let v` in `gp.rs`

- **Files in scope.** Must edit: `crates/engine/src/cmd/gp.rs` only.
- **Spec.** `commands()` (~line 14) builds `let v = vec![...]` and returns a bare `v` (~line 49).
  Return the `vec![...]` directly. No other change.
- **Anchors.** `fn commands`.
- **Acceptance.** `cargo test -p rustgis-engine commands`. Also `cargo test -p rustgis-engine
  hostile_params_never_panic`.

### 11.2.1 desktop R runs in the background (Spec 1)

- **Files in scope.** Must edit: `crates/engine/src/cmd/gp_r.rs`, `crates/ui-egui/src/lib.rs`
  (the hook wiring only). Can create: none. Read only: `crates/engine/src/cmd/gp.rs` (the
  finish path), `crates/ui-egui/src/lib.rs` (the geolibre hook).
- **Spec.** `run_r` (`gp_r.rs` ~line 197) runs `default_runner().run` on the calling thread. The
  fix, per D1: the run is queued through `r_start`, then the desktop host runs it on a thread and
  calls `r_complete` and `gp.finish` when it returns. Mirror how the geolibre desktop path hands
  its job to a worker thread. The web build keeps its "can't run yet" refusal.
  The engine does not run R itself after this change.
- **Anchors.** `fn run_r`, `fn r_start`, `fn r_complete`, `fn r_reset`; `Services` field
  `r_run` (`ui-egui/src/lib.rs` ~line 199); `external_start` and `geolibre_run` in `ui-egui`
  for the reference pattern; `gp.finish` in `gp.rs`.
- **Hazards.** The UI thread must never block. Keep the `r_running` single-run guard. A dropped
  project already clears the queue through `r_reset`; keep that. This is the riskiest task:
  escalate to a mid-tier model after one failed attempt (see the ladder).
- **Constraints.** No new deps. No `std::thread` in the engine (engine builds for wasm); the
  thread lives in `ui-egui`, which is native-only for the desktop path.
- **Acceptance.** `cargo test -p rustgis-engine r_` (existing R queue tests still pass, and the
  inline-run test changes to assert `gp.runR` returns a queued job, not a result). Add a test:
  the queue holds one run while another is in progress. `cargo build -p rustgis` (desktop).

### 11.2.2 one R-run predicate (Standards)

- **Files in scope.** Must edit: `crates/engine/src/cmd/plugins.rs`,
  `crates/engine/src/cmd/workflows.rs`, `crates/engine/src/cmd/items.rs`,
  `crates/assistant/src/tools.rs`, `crates/assistant/src/mcp.rs`.
- **Spec.** Per D3, keep the `r:` guard, but compute "is this an R run" in one place. The four
  sites today: `plugins.rs` ~lines 305–312 (`user_only_call` + `runs_r`), `assistant/src/tools.rs`
  ~line 153 and ~189, `assistant/src/mcp.rs` ~lines 187–193, `workflows.rs` ~lines 285–288
  (calls `is_r_tool` twice), and `items.rs` ~line 614. Make one function that answers the question
  and have each site call it. In `workflows.rs`, call it once per step.
- **Anchors.** `R_TOOL_PREFIX`, `is_r_tool`, `runs_r`, `user_only_call` in `plugins.rs`
  (~lines 259–274).
- **Constraints.** No behaviour change: the same tools are refused over MCP, and the same
  workflow steps are marked. Do not touch `ui-egui`.
- **Acceptance.** `cargo test -p rustgis-engine plugins` and `cargo test -p rustgis-engine
  workflows`, then `cargo test -p rustgis-assistant mcp`. Existing tests must pass unchanged
  (only call sites change).

### 11.3.1 one notice and error path (Standards)

- **Files in scope.** Must edit: `crates/r/src/lib.rs`, `crates/r/src/notices.rs`,
  `crates/ui-egui/src/rnotices.rs`, `crates/engine/src/cmd/gp_r.rs`, `crates/engine/src/cmd/gp.rs`.
- **Spec.**
  1. `RRunnerError::NotFound` (`lib.rs` ~line 70) duplicates `RNotice::NotFound`. Make it
     `RRunnerError::Notice(RNotice::NotFound)`. Remove the separate variant. Update `gp_r.rs`
     ~line 125 so both paths map through one notice.
  2. The `RNotice` match is repeated in `notices.rs` (`english`), `rnotices.rs` (`text`) and
     `gp_r.rs` ~lines 124–126 (`r_failure`). Keep the two translator matches (each is the
     translation table). Remove the one in `gp_r.rs` by calling the notice's text.
  3. "The run was dropped (its project was closed, or too many runs were waiting)" appears in
     `gp.rs` and `DROPPED_R_RUN` in `gp_r.rs`. Keep one `pub(crate) const`, used by both.
- **Anchors.** `RRunnerError`, `DROPPED_R_RUN`, `fn r_failure`, `fn english`, `fn text`.
- **Constraints.** No change to the message text the user sees in English. Hostile input stays
  covered by the existing tests.
- **Acceptance.** `cargo test -p rustgis-r` and `cargo test -p rustgis-engine r_` and
  `cargo test -p rustgis-ui-egui rnotices` (if present). Then `cargo xtask ci` on the branch.

---

## Hazard notes (from recon)

- `crates/engine/src/tests.rs` (~3000+ lines) and `crates/ui-egui/src/lib.rs` (2816 lines) are
  large. Workers enter them by grep anchor, never a whole-file read.
- `planning/tasks/10.*.todo.md` are the previous orchestration's files. Do not edit them.
- `local main` is 196 commits behind `origin/main`. Diffs and worktrees start from
  `feature/engine-r-core-main`.
- Commit rules: conventional subject, one sentence, no orchestration or wave numbers, no AI
  attribution trailers (AGENTS.md; the user's commit-trailers memory).
- `cargo xtask ci` before every commit (AGENTS.md). Wave-end runs are serial (no concurrent suite).
- R is not installed in every environment. Harness tests that need Rscript must skip, and the
  worker reports the skip.

## Wave-end checks (orchestrator, after each wave)

1. Serial: `cargo test -p rustgis-r -p rustgis-engine -p rustgis-assistant -p rustgis-ui-egui`.
2. `cargo test -p rustgis-engine hostile_params_never_panic` (and the analysis equivalent).
3. After wave 3: `cargo xtask ci`, `cargo build --target wasm32-unknown-unknown -p rustgis-web`.
