# Master Plan: engine-level R runtime

Source: RFC 001 (`planning/rustgis-engine-r-runtime.md`, raw), corrected by the fact base
`planning/research/r-runtime-findings.md` (cited below as F§n / F-Cn / F-En). Decisions agreed in the
grilling session of 2026-10-14.

Each phase is planned with `/plan-wave` into `planning/EXECUTION_PLAN_N.md` (numbering continues
from Plan 1) and executed with `/wave-orchestration`. Phases run strictly in order.

## Exit gate (every phase)
- `cargo xtask ci` green (fmt, clippy `-D warnings`, tests, layers).
- `cargo build --target wasm32-unknown-unknown -p rustgis-web` green.
- The phase's own tests; every new command/tool exercised by `hostile_params_never_panic`
  (engine or analysis); every crash fix gets a regression test.
- Docs for what the phase shipped are updated in the same phase.
- No real R execution may land before P0's security gating (F§8).

## Names (RFC → repo)
Plan 1 names stand (repo convention, Plan 1 D2). `/plan-wave` translates RFC text through this table.

| RFC | Repo |
|---|---|
| `crates/r-runner` | `crates/r` (package `rustgis-r`, L3) |
| feature `r-runtime` (default on) | feature `r`, enabled per app (like `geolibre`) |
| `r.runScript` | `gp.runR` |
| `r:*` tools via `gp.run` | same (`r:` prefix, `register_external`) |
| `RJobRequest` / `RJobResponse` | `RExecutionInput` / `RExecutionOutput` (extended in P1) |
| `Services::r_run` (engine trait) | `Services.r_run` callback field in L6 `crates/ui-egui/src/lib.rs` |
| `RUSTGIS_RSCRIPT_PATH` | `RUSTGIS_RSCRIPT` |
| `catalog.rs` / `rustgis-cli parity` | does not exist (F-C9); ignore |

## RFC corrections that bind the phases
- `Services` is an L6 struct of optional callbacks; CLI and Python have none (F-C1, F-C2).
- Native GeoLibre runs synchronously and blocks the desktop UI; `gp.finish` takes only `{"id"}`;
  `external_start/external_complete` are GeoLibre-only (F-C3–C5).
- No L2 change needed: `rustgis_analysis::register_external` (F-C8). No Arrow (F-C15).
- Policy enforcement goes through `NEVER` / `user_only` / `refused` in
  `crates/engine/src/cmd/plugins.rs`, not per-host code (F-C10, F-C11).
- webR is v0.6.0 (R 4.6), not 0.5.4 (F-E1). The RFC's JS bridge and bootstrap R are defective
  (F§4); rewrite, don't copy.
- `--vanilla` drops `~/.Renviron` and user package libraries (F-E11).

## Decisions
| # | Decision |
|---|---|
| D1 | Plan 1 is Phase 0's base; its final gate is re-run there. |
| D2 | Plan 1 names kept (table above). |
| D3 | `r` feature is opt-in per app: desktop, CLI, Python enable it in P3; web in P6. |
| D4 | Scope: native + browser + built-in `r:` tools; browser and tools last. |
| D5 | One `EXECUTION_PLAN_N.md` per phase; exit gate above. |
| D6 | User R scripts are saved in the project (editor reopens them) plus Open/Save `.R`. Opening a project never runs R. |
| D7 | One R job at a time per session; later runs queue. |
| D8 | Timeout 120 s default, `timeout_s` clamped to 1 s–24 h; Cancel from P4. Cancelled/timed-out jobs leave the project untouched. |
| D9 | `Rscript --no-save --no-restore --no-init-file` (keeps `.Renviron`). Discovery: `$RUSTGIS_RSCRIPT`, PATH (absolute entries), default install locations; "Locate Rscript…" pref like `gp.setGeolibre`. |
| D10 | Harness: requires `jsonlite` (safe: geolibre-r imports it, and it is in webR's repo, F-E9); `sf` used when installed, base-R table fallback otherwise; a clear install message when a script needs geometry and `sf` is missing. |
| D11 | CLI and Python block: `gp.runR` returns after the result is applied. |
| D12 | R tests skip with a message when R is absent; a separate CI job (`r-lib/actions/setup-r` + `sf`) runs them from P3; webR smoke test in P6. |
| D13 | Commit `MASTER_PLAN.md` and execution plans; commit a cleaned RFC to `docs/rfcs/001-engine-r-runtime.md` (images dropped, citations kept, erratum header pointing to the findings and the names table). Raw RFC stays untracked. |
| D14 | Security: `gp.runR` and every `r:` tool are user-only. Plugins never; MCP refuses; the Assistant asks every time (no "allow for this conversation"). Lands in P0. |
| D15 | R steps allowed in models, tasks and `script.run` when the user runs them; imported models refuse R steps like file-touching steps (`workflows.rs:279`). |
| D16 | Desktop runs R on a background thread via `Services.r_run`, polled like `poll_geolibre`. |
| D17 | Vector wire format: FlatGeobuf for geometry layers, CSV for geometry-less tables. Web falls back to GeoJSON if webR's GDAL lacks the FlatGeobuf driver (checked in P6). |
| D18 | Rasters: original file bytes when available, else single-band `geotiff::write`. |
| D19 | No Arrow. Output tables are CSV files listed in the manifest. |
| D20 | Selection rows in the manifest are 1-based (R-native); the host converts to 0-based and bounds-checks. |
| D21 | CRS: inputs EPSG:4326; `rgis_write_layer` transforms outputs to 4326 and refuses CRS-less output with out-of-range coordinates. |
| D22 | Size caps: `MAX_UPLOAD_BYTES` (512 MiB native / 256 MiB wasm) per input and output; script ≤ 1 MB. |
| D23 | `LANGUAGE` set to the interface language. |
| D24 | Built-in `r:` tools: embedded `.R` files (`include_str!` in `crates/r`) with a JSON parameter header, registered via `register_external`, shown as an "R ›" toolbox. |
| D25 | Missing R packages: desktop shows install instructions, never installs; web `webr::install()` on demand, with a download notice. |
| D26 | Output streaming: after-finish in P3; live log in P4. |
| D27 | webR 0.6.0 pinned from jsDelivr, same trust model as geolibre-wasm (`apps/rustgis-web/src/geolibre.rs:12`). |
| D28 | Web packages: per-session install with IndexedDB cache (forces PostMessage channel). Online-only is acceptable. |
| D29 | Plots out of scope. |
| D30 | One async job path shared by GeoLibre and R (`ExternalJob` enum, shared `MAX_ASYNC_RUNS`, one `gp.finish`); D7 queue on top. |
| D31 | Keep the sync `RScriptRunner` trait (Plan 1 D1); native impl runs on the desktop worker thread or inline (CLI/Python). Web bypasses it via `Services.r_run`; drop the wasm `WebRRunner` stub if unused. |
| D32 | Run history gets a `Running` status and a live log buffer (P4), shared with GeoLibre. |
| D33 | R Script pane: editor with our own simple R syntax colouring, input-layer picker, Run/Cancel, log, Open/Save `.R`. No run-selection. |
| D34 | Docs: new `docs/user-guide/r.md`; `docs/architecture.md`, `docs/mcp.md`, `docs/plugins.md`, AGENTS.md L3 row updated by the phase that changes them. |

## Phases

### P0 — Close out Plan 1 and gate R
- Re-run Plan 1's final gate on HEAD; record in `STATE_1.md`.
- Add `gp.runR` to `NEVER`; tool-level refusal of `r:` tools in `refused`; Assistant approval every
  time; MCP refusal (D14). Tests for each host.
- Imported models refuse R steps (D15).
- Clean RFC copy to `docs/rfcs/001-engine-r-runtime.md` (D13); docs updates to `docs/mcp.md`,
  `docs/plugins.md`.
- Exit: gate + tests showing plugins/MCP refuse and the Assistant asks.

### P1 — Wire contract (pure Rust, wasm-clean)
- Extend `RExecutionInput`/`RExecutionOutput`: typed layer inputs (FGB / CSV / raster bytes),
  `timeout_s`, `lang`.
- `params.json` and `out/manifest.json` schemas; bootstrap harness `bootstrap.R` (ours, embedded)
  with `rgis_read_layer`, `rgis_write_layer`, `rgis_write_table`, `rgis_select`, `rgis_log`, a
  top-level `tryCatch` writing a failure manifest (D10, D19–D21).
- Manifest → `analysis::Output` conversion under `catch_unwind`; missing/invalid manifest = failure.
- Size caps (D22).
- Exit: gate + round-trip tests from fixture manifests (no R needed); hostile manifest tests.

### P2 — Engine async path
- Generalize `external_start` / `external_complete` / `Session.started` / `gp.finish` into a
  runner-agnostic job path (D30); GeoLibre behaviour unchanged.
- `r_start` stages inputs through `map_inputs` (streamed layers, selections) and the P1 writers.
- One-at-a-time R queue (D7). `gp.runR` and `r:` tools mutate only through `gp.finish` (single undo step).
- Exit: gate + engine tests driving a fake R completion through start → complete → finish for
  success, failure, and queueing; GeoLibre async tests still green.

### P3 — Native runner
- `crates/r/src/native.rs`: discovery (D9), scratch dir and `output_within` patterns from
  `crates/geolibre/src/{native,runner}.rs`, timeout + kill, cleanup, `LANGUAGE` (D23),
  `CREATE_NO_WINDOW` on Windows.
- "Locate Rscript…" command and pref; missing-R / missing-package notices (D25).
- CLI and Python block (D11); desktop, CLI, Python enable `r` (D3).
- R-dependent tests skip without R; new CI job with R + `sf` (D12).
- Docs: `docs/user-guide/r.md` (install, `gp.runR`, harness API), architecture + AGENTS.md rows.
- Exit: gate + R CI job green.

### P4 — Desktop UI
- `Services.r_run` on desktop: background thread, polling, Cancel (D16, D8).
- Run history `Running` status + live log (D32), shared with GeoLibre.
- R Script pane (D33); scripts saved in the project (D6).
- `tl!` strings in all 15 `crates/ui-egui/src/i18n/*.tsv` catalogs; `ui_shot` screenshots
  reviewed (images go to rustgis-assets, not this repo).
- Exit: gate + `ui_shot` review.

### P5 — Built-in `r:` tools
- Tool manifest format (JSON header in each embedded `.R`), registration via `register_external`,
  "R ›" toolbox (D24). First set from the RFC: `r:local-moran`, `r:kriging`, `r:point-density`,
  `r:spatial-regression` (final list and package requirements decided in `/plan-wave`).
- Analysis hostile-params test must not spawn real R.
- Exit: gate + per-tool tests in the R CI job.

### P6 — Browser (webR)
- Real bridge in `apps/rustgis-web/src/r.rs` (inline-JS + `spawn_local` + repaint, pattern
  `geolibre.rs`): lazy init, PostMessage channel, MEMFS staging with `Uint8Array` transfer,
  `setwd(jobDir)`, condition text captured, timeout via `close()` + respawn (D27, D28).
- Verify FlatGeobuf driver in webR's GDAL; GeoJSON fallback if absent (D17).
- `webr::install()` on demand with notice; IndexedDB cache (D25, D28).
- Web enables `r` (D3); webR smoke test (D12); user guide web section.
- Exit: gate + smoke test.

## Future work (not planned)
- User-defined R tools (script + parameter manifest).
- Plots: PNG files in `out/` shown in a viewer.
- Prebuilt webR package images (WORKERFS) for the core spatial set.
- Multiband GeoTIFF writing.
- A nanoarrow/geoarrow data interface, possibly provided through
  [geolibre-r](https://github.com/opengeos/geolibre-r) (an R package on CRAN, likely a default
  install alongside RustGIS's R support; no Arrow support today).
- Arrow for large attribute tables (native-only, feature-gated) if FlatGeobuf/CSV becomes a bottleneck.
