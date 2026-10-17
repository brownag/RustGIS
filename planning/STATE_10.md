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
| 10.3.1 typed R notices (+ fold harness.rs) | 3 | pending |
| 10.4.1 gp_r submodule | 4 | pending |
| 10.4.2 move manifest panic guard | 4 | pending |
| 10.4.3 hostile coverage with R | 4 | pending |
| 10.5.1 final gate + PR prep | 5 | pending |

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
