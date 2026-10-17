# State — Orchestration 10 (R core PR)

Branch: `feature/engine-r-core` (to be created by 10.1.1 from `main` = be44b0b)
Source: `feature/engine-r-runtime` commits `ac02a1d..7df1a29` (tip `59f5ca3`), planning stripped.

## Baseline (2026-10-16, at 28f2c78)
- [x] `cargo xtask ci` — pass (fmt, clippy, tests, layers ok)
- [x] `cargo test -p rustgis-r -p rustgis-engine --features r` (Rscript + jsonlite + sf) — 186 passed, 0 failed
- [x] `cargo build --target wasm32-unknown-unknown -p rustgis-web` — pass
- [x] `git diff 59f5ca3 HEAD --stat -- . ':!planning'` empty — yes (46 commits ported, 26 planning-only skipped; ecc8c87 conflict was planning-only)

## Wave board
| Task | Wave | Status |
|---|---|---|
| 10.1.1 branch + baseline | 1 | done |
| 10.2.1 drop webR shim, keep hook | 2 | done |
| 10.2.2 trim docs + RFC erratum | 2 | done |
| 10.2.3 lang as gp.runR param | 2 | done |
| 10.3.1 typed R notices (+ fold harness.rs) | 3 | done |
| 10.4.1 gp_r submodule | 4 | done |
| 10.4.2 move manifest panic guard | 4 | done |
| 10.4.3 hostile coverage with R | 4 | done |
| 10.5.1 final gate + PR prep | 5 | done |

## Task reports

## 10.2.1
- **Status:** done (not committed).
- **Files:** `crates/r/src/lib.rs` 14-19, 34-39 (mod web and WebRRunner re-export removed), 77-81 (`default_runner` now `#[cfg(not(wasm32))]`, native only); `apps/rustgis-web/src/main.rs` 18-21 (`mod r;` removed); `apps/rustgis-web/src/web.rs` 608 (`r_run: None`); deleted `crates/r/src/web.rs`, `apps/rustgis-web/src/r.rs`. `tests.rs` unchanged.
- **Tests:**
  - `cargo test -p rustgis-r`: 72 passed, 0 failed.
  - `cargo build --target wasm32-unknown-unknown -p rustgis-web`: pass (exit 0).
  - `cargo clippy -p rustgis-r --target wasm32-unknown-unknown -- -D warnings`: pass.
  - `cargo fmt -p rustgis-r -p rustgis-web`: no changes. `cargo clippy -p rustgis-r -p rustgis-web -- -D warnings`: pass.
- **Deviations:** none. Step 1 baseline skipped per coordinator.
- **Gotchas:**
  - Engine `run_r` already has a wasm arm ("R scripts can't run in this build yet"), so `gp.runR` fails cleanly on the web.
  - Nothing in ui-egui reads `Services::r_run`; the hook is unused, kept per D2.
  - `RRunnerError::Unsupported` has no producer now; kept as instructed.
  - `apps/rustgis-web/Cargo.toml` still enables `features = ["r"]` (H4, out of scope).

## 10.2.2
- Status: done (docs only, not run: cargo xtask ci). Not committed.
- Files: `docs/user-guide/r.md` 1-80 (was 135; full rewrite); `docs/rfcs/001-engine-r-runtime.md` 500-507 (new `## Erratum`; body untouched).
- Checks: r.md `wc -l` = 80 (<=80 ok). `grep -c '^## Erratum'` RFC = 1 (ok). `grep -c Erratum` = 2 (existing line 3 note plus new heading). No `vanilla` or `catalog.rs` in r.md. Anchor `#limits` resolves to `## Limits`.
- Verified in code: `crates/r/src/native.rs:88` uses `--no-save --no-restore --no-init-file`; `gp.runR` params `script`, `inputs`, `args`, `timeout_s` (gp.rs:47); `crates/engine/src/catalog.rs` absent.
- Deviations: none from approved plan. Q2 answered by dropping the install-time `terra` mention, the message cap, and the file-size limits.
- Gotchas: `gp.rs:147` still reads `s.lang` (10.2.3 changes this); r.md documents `lang` as an optional param as the spec requires. The RFC body still says `--vanilla` by design (erratum only). `docs/user-guide/index.md:13` still says "script harness and limits" (out of scope).

## 10.2.3
- Status: done (not committed). Plan at planning/tasks/10.2.3.todo.md.
- Files: crates/engine/src/cmd/gp.rs (44-56 fmt-only; 48 params string; 82-83 RParams.lang; 115-116 parse; 149 `lang: p.lang`); crates/engine/src/lib.rs (Session.lang field and initialiser removed); crates/engine/src/tests.rs (~827-838 test passes `lang` in params); crates/ui-egui/src/lib.rs (run(): gp.runR gets `with_lang`; sync block removed; `with_lang` helper + `lang_tests` module appended at end).
- Tests: `cargo test -p rustgis-engine --features r` 114 passed, 0 failed (includes gp_run_r_passes_the_interface_language_to_r, hostile_params_never_panic). `cargo test -p rustgis-ui-egui --features r` 15 + 15 passed, 0 failed (includes with_lang_fills_in_a_missing_language_only).
- Gates: `grep -rn 'session.lang\|s\.lang' crates/` empty. `cargo clippy -p rustgis-engine -p rustgis-ui-egui --features r -- -D warnings` clean. `cargo fmt --all` run.
- wasm: `cargo check -p rustgis-engine --target wasm32-unknown-unknown` ok; `cargo build --target wasm32-unknown-unknown -p rustgis-web` ok.
- Deviations: cargo fmt also re-indented the pre-existing gp.rs `commands()` block (44-56), whitespace only. Step 8 done as a pure helper unit test, not a ui-egui R run. Step 9: no new hostile cases added.
- Gotchas for 10.3.1: `RParams.lang` now carries the language into `r_start` (gp.rs ~149); the RNotice work should not touch it. `r_complete` and PendingRun are unchanged. Assistant `gp.run` path gets no lang (falls back to English; gp.runR is user-only).

Wave 2 gate (at 28bdc53): `cargo xtask ci` pass; `cargo test -p rustgis-r -p rustgis-engine -p rustgis-ui-egui --features r` 216 passed 0 failed; wasm build pass.

## 10.3.1
- Status: done (not committed). Plan at planning/tasks/10.3.1.todo.md (answers from the approval applied).
- Files: crates/r/src/notices.rs (RNotice, ALL_NOTICES, code/english/from_code; sync test); crates/r/src/lib.rs (inline `harness` module; RRunnerError::Notice; From impls); crates/r/src/harness.rs deleted; crates/r/src/bootstrap.R (.rgis_need takes a code, rgis_notice condition, "notice" key in manifest); crates/r/src/manifest.rs (convert_output returns RRunnerError; notice branch); crates/r/src/manifest_tests.rs, harness_tests.rs (typed asserts, new notice test); crates/engine/src/lib.rs (Notice.r_notice, Session.r_notice, PendingRun.r_notice, guarded takes it); crates/engine/src/cmd/gp.rs (r_failure, r_complete, finish, sync path); crates/engine/src/tests.rs (a_failed_r_run_keeps_its_typed_notice); crates/ui-egui/src/rnotices.rs (text_of, text); crates/ui-egui/src/chrome.rs 211, 545.
- Tests: `cargo test -p rustgis-r` 74 passed, 0 failed. `cargo test -p rustgis-engine --features r` 115 passed, 0 failed (includes hostile_params_never_panic). `cargo test -p rustgis-ui-egui --features r` 15 + 15 passed, 0 failed.
- Gates: `grep strip_prefix(R_FAILURE_PREFIX)` empty; no .tsv diff; clippy -D warnings clean (r, engine, ui-egui with --features r; engine+ui-egui without r); `cargo build --target wasm32-unknown-unknown -p rustgis-web` ok.
- Deviations: harness.rs folded as an inline module (job.rs imports it); sync test checks R-side codes only (not_found is native); side field taken in `guarded`, not `Session::run`; `cargo fmt -p` reformatted the 3 rustgis-r test/src files.
- Gotchas for 10.4.x: gp_r submodule move must carry the `r_notice` cfg fields (PendingRun literal at gp.rs ~520 is geolibre-only). `convert_output` now returns RRunnerError; the catch_unwind arm maps to Failed (10.4.2 still owns the guard). Hostile tests green. Notice text stays English, typed via `r_notice`.

Wave 3 gate (at f26cd9d): `cargo xtask ci` pass; R/engine/ui-egui tests with `--features r` 219 passed 0 failed; wasm build pass; no .tsv diff.

## 10.4.1
- Status: done (not committed). Plan at planning/tasks/10.4.1.todo.md (approved with amendments: keep all gates, no allow attrs, trim_started keeps its read, extra re-exports R_FAILURE_PREFIX and r_params).
- Files: crates/engine/src/cmd/gp_r.rs (new, 265 lines: commands(), RRun, R_FAILURE_PREFIX, DROPPED_R_RUN, RParams, r_params, r_start, r_failure, r_complete, run_r, rscript_status, set_rscript, RJob, RRunStart, r_reset); crates/engine/src/cmd/gp.rs (re-export line 11; RJob path in ExternalJob line ~170; MapInputs, MAX_ASYNC_RUNS, trim_started, map_inputs, now_ms, finish now pub(super); 563 lines); crates/engine/src/cmd/mod.rs (`pub(crate) mod gp_r` cfg r; gp_r::commands() extended after gp::commands()); crates/engine/src/lib.rs (reset block -> `cmd::gp_r::r_reset(self)`, cfg r).
- Tests: `cargo test -p rustgis-engine --features r` 115 passed, 0 failed. `--features geolibre,r` 117 passed, 0 failed. `cargo test -p rustgis-engine` 100 passed, 0 failed.
- Gates: `cargo clippy -p rustgis-engine --features r -- -D warnings` clean; `cargo clippy -p rustgis-engine -- -D warnings` clean; `cargo check -p rustgis-ui-egui --features r` ok; `cargo check -p rustgis-engine --features r --target wasm32-unknown-unknown` ok, no warnings; `cargo fmt --all -- --check` clean.
- grep: `grep -c 'feature = "r"' gp.rs` = 10 (amended limit). Remaining: re-export (10); MAX_ASYNC_RUNS (164); ExternalJob enum (168); R variant (172); trim_started (178); trim running read/None (183, 185); external_complete R arm (244); PendingRun r_notice literal (259); finish() r_notice block (279).
- Deviations: MapInputs and finish widened to pub(super); `finish` import in gp_r gated not(wasm) to avoid an unused-import warning on wasm; registry order unchanged; the R registrations were rewritten as one vec! (same entries, same order).
- Gotchas: `cmd::gp_r` must be `pub(crate)` for lib.rs's reset call. gp_r.rs changed on disk once mid-task (someone else's edit, not reverted by me). 10.4.3 tests import `r_params` and `R_FAILURE_PREFIX` via gp; both are re-exported.

## 10.4.2
- Status: done (not committed). Plan at planning/tasks/10.4.2.todo.md (approved: fallback A).
- Files: crates/r/src/manifest.rs (comment only, above the catch_unwind in convert_output, ~line 26). No code change; the guard stays.
- Tests: `cargo test -p rustgis-r` 74 passed, 0 failed. `cargo test -p rustgis-ui-egui --features r` 15 + 15 passed, 0 failed.
- Gates: `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --check -p rustgis-r` clean; `cargo build --target wasm32-unknown-unknown -p rustgis-web` ok.
- Deviations: TODO 4 skipped (no panic-injection hook, per coordinator).
- Gotchas: H3 premise was wrong. panes.rs:881 is the geolibre arm only; native R completes inside Session::run (gp.rs:255). Session::run's guard is inline (engine/src/lib.rs:559), not a helper. Desktop background R jobs (per coordinator) would call r_complete outside the guard, so the inner guard stays.

## 10.4.3
- Status: done (not committed). Plan at planning/tasks/10.4.3.todo.md (approved; coordinator answers applied: r_notice asserted, no gp.setRscript junk).
- Files: crates/engine/src/tests.rs (only file changed): junk entries 2 added to hostile array (lines 589-591 approx.); new tests gp_run_r_params_reject_or_clamp_junk (818), gp_run_r_rejects_junk_before_any_run (829), gp_run_r_without_rscript_is_an_error_not_a_panic (847), all `#[cfg(feature = "r")]`).
- Tests: `cargo test -p rustgis-engine --features r hostile` 1 passed, 0 failed. `cargo test -p rustgis-engine hostile` 1 passed, 0 failed. `cargo test -p rustgis-engine --features r` 118 passed, 0 failed (was 115 + 3 new).
- Gates: `cargo fmt -p rustgis-engine` clean (only tests.rs modified); `cargo clippy -p rustgis-engine --features r --tests -- -D warnings` clean.
- Deviations: none from approved plan. Spec's `gp.locateR*` does not exist; real R ids are gp.runR and gp.setRscript (ui.locateRscript is UI-only).
- Gotchas: CHOSEN/FOUND in r/discover.rs are process-wide statics, so no test calls choose(). "No runner" is tested via RRunnerError::NotFound through r_complete + gp.finish, not via s.run("gp.runR") (approved). Hostile `{"path": 7}` for gp.setRscript would reset the global choice; deliberately not added.
- No `skipped:` lines were required: the new tests do not launch R; the existing R-launching tests ran (no skip seen in the 118-passed run output).

Wave 4 gate (at 6218ea1): `cargo xtask ci` pass; R/engine/ui-egui tests `--features r` 222 passed, `--features geolibre,r` 224 passed, 0 failed; wasm build pass; gp.rs has 10 `feature = "r"` lines.

## 10.5.1
- Final gate = wave 4 gate at 6218ea1 (no code changed after). planning/ holds only MASTER_PLAN.md, *_10.md and tasks/10.*. `git diff main...HEAD -- . ":!planning"`: 72 files, +4416/-71. PR body drafted; push/PR await the user.
