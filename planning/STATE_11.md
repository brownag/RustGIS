# STATE 11 -- R core review fixes

Resume point for the orchestrator. Plan: `planning/EXECUTION_PLAN_11.md`.
Prompt: `planning/PROMPT_TEMPLATE_11.md`. Branch: `feature/engine-r-core-main`.

## Baseline (record before wave 1)

- [x] `cargo xtask ci` on the branch: RED before wave 1 (2026-10-18). Clippy `-D warnings` fails in
  `crates/map/src/globe.rs:439` (`nonminimal_bool`), outside plan 11. Log: scratchpad `baseline_ci.log`.
- [x] `cargo test -p rustgis-r`: 74 passed, 0 failed.
- [x] `cargo test -p rustgis-engine`: 149 passed, 1 failed, 1 ignored. Pre-existing failure:
  `tests::flatgeobuf_and_geoparquet_by_address_are_read_in_place` ("the layer's source isn't open").
- [x] `cargo test -p rustgis-assistant`: 176 passed, 0 failed.
- [x] `cargo build --target wasm32-unknown-unknown -p rustgis-web`: ok.
- [x] Rscript on PATH in this environment: y (`/usr/bin/Rscript`), so harness tests run, not skip.

## Wave board

| Wave | Task | Status |
|---|---|---|
| 1 | 11.1.1 output bounds (C1) | done `3dbdba2` |
| 1 | 11.1.2 empty manifest is valid (C2) | done `d1a0c55` |
| 1 | 11.1.3 bootstrap setup failures (Spec 3) | done `54e70bd` |
| 1 | 11.1.4 Rscript-not-found instructions (Spec 2) | done `8b1d1cc` |
| 1 | 11.1.5 `ready()` (was "delete") | cancelled: reversed D8, `ready()` is live |
| 1 | 11.1.6 doc drift | done `1464a61` |
| 1 | 11.1.7 inline `let v` in gp.rs | done `bdfa898` |
| 2 | 11.2.1 desktop R runs in the background (Spec 1) | done `0701176` (CLI re-scoped in; no test covers the desktop thread path) |
| 2 | 11.2.2 one R-run predicate | done `352eebc` |
| 3 | 11.3.1 one notice and error path | done `c15a5f3` (`DROPPED_RUN_WHY` shared; two messages kept byte-identical) |
| 4 | Wave-end verification (serial suites, xtask ci, wasm build) | done, with caveats below |

Wave 4 (2026-10-18), on `feature/engine-r-core-main` at `a21bd2d`:
- `rustgis-r` 80 passed. `rustgis-assistant` 176 passed. `rustgis-engine` 149 passed, 1 failed: the
  pre-existing `flatgeobuf_and_geoparquet_by_address_are_read_in_place`.
- `rustgis-ui-egui` 81 passed, 0 failed, unskipped (the `mapgpu` parity tests pass alone in 0.43 s).
- `cargo test -p rustgis-engine --features r hostile_params_never_panic` and `rustgis-analysis hostile`: passed.
- `cargo build -p rustgis-cli`, `cargo build --target wasm32-unknown-unknown -p rustgis-web`: pass.
- `cargo fmt --all --check`: failed on two lines from this work; fixed in `a21bd2d`.
- Clippy `-D warnings`: fails in `rustgis-map` (pre-existing, `globe.rs:439`). With that one lint
  allowed, the touched crates are clean.
- An earlier stall in `mapgpu::` parity tests happened while other cargo jobs ran in parallel. Alone,
  they pass. Cause of the stall not determined.
- Caveat: no test covers the desktop R background thread (11.2.1).

Wave 1 end (2026-10-18): serial suites green except the pre-existing `flatgeobuf` engine test;
`rustgis-ui-egui` 81 + integration tests all pass. `cargo xtask ci` still red on `rustgis-map` clippy
(not plan 11).

## Decision log

- 2026-10-18 D1 desktop R in background now; cancel stays P4.
- 2026-10-18 D2 `Services::r_run` kept for webR (P6).
- 2026-10-18 D3 `r:` tool guard kept; call sites consolidated.
- 2026-10-18 D4 `ok:true` with neither layers nor selection is valid.
- 2026-10-18 D5 `MAX_TOTAL_BYTES` = 512 MiB, summed before reading.
- 2026-10-18 D6 missing-Rscript notice gives install instructions.
- 2026-10-18 D7 bootstrap failure manifest covers setup.
- 2026-10-18 D8 `discover::ready()` deleted. REVERSED: `ready()` checks `jsonlite` and `sf` and is
  the engine's R test gate (`r_ready` in `engine/src/tests.rs`). It is not dead code. Kept.

## Task reports

<!-- Workers append under ## {{id}}, <= 15 lines each. -->
