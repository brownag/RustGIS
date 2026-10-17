# Execution Plan 10 — R runtime core PR (`feature/engine-r-core`)

## Goal & Scope

**Goal.** Cut a lean, reviewable PR that lands the R runtime *engine core* on a new branch
`feature/engine-r-core` from `main`, and fix the issues the 2026-10-16 two-axis review raised
against that core. User's words: "refine to make a viable engine with less focus on minutia and
specifics for a PR to get this functionality in"; "a new branch that will just be the 'core'".

**In scope**
- Port the non-planning commits `ac02a1d..7df1a29` (Plans 5–6; tip `59f5ca3`) to the new branch.
- Fix the review findings that apply to that core:
  - drop the webR placeholder shim, keep the `r_run` Services hook;
  - typed R notices instead of matching English text;
  - interface language passed as a `gp.runR` param, not held in `Session`;
  - fold `harness.rs`, drop the inner `catch_unwind` (see hazard H3);
  - collect gp.rs's R queue code into one submodule;
  - confirm hostile-params coverage with `--features r`;
  - trim `docs/user-guide/r.md` to one page and add an erratum to the RFC;
  - prune `planning/` to `MASTER_PLAN.md` + this plan's files.

**Out of scope (follow-up PRs, stay on `feature/engine-r-runtime`)**
- P4 desktop UI: `rpane.rs`, `rjobs.rs`, `rsyntax.rs`, `cmd/rscripts.rs`, History "Running".
- P5/Plan 9 `r:` tools (`tools/*.R`, `tool.rs`, `tools.rs`) and all tool-specific fixes
  (Moran's I, point density, spatial regression, statistics selection, extent clipping), and the
  strict CI check that requires `spdep`/`gstat`/`spatialreg`.
- `crates/testkit` temp-folder cleanup; Geoprocessing ribbon overlap fix (`56d6344`).
- The real webR bridge (P6).
- `RunFailure` dedupe: `RunFailure` does not exist at `59f5ca3` (added later), so nothing to do.

**Verbatim constraints**
- "Keep the hook, drop the shim" (web path).
- "Keep RScriptRunner trait".
- AGENTS.md: never crash, never break wasm, interface text translatable, `cargo xtask ci` green.

## Decision log (2026-10-16)
| # | Decision | Why |
|---|---|---|
| D1 | Cherry-pick `ac02a1d..7df1a29`, skipping `docs(planning)` commits; strip `planning/` from mixed commits | keeps readable history and authorship |
| D2 | Web: keep `Services::r_run` hook (`crates/ui-egui/src/lib.rs:109`), delete `apps/rustgis-web/src/r.rs`, `crates/r/src/web.rs`, `WebRRunner`; web sets `r_run: None` | extension point for P6 without dead code |
| D3 | Notices: `enum RNotice { NotFound, NeedJsonlite, NeedSf, NeedTerra }` in `rustgis-r`; bootstrap.R emits a stable code; the variant travels to the UI, which does `match → tl!` | no matching on English text (docs/i18n.md) |
| D4 | Language: optional `lang` param on `gp.runR`; the UI adds it when dispatching; remove `Session.lang` | the engine holds no front-end state |
| D5 | Cleanups: fold `harness.rs`, drop inner `catch_unwind` (subject to H3), **keep** `RScriptRunner` trait | the trait is the extension point next to the kept hook |
| D6 | Move R queue code (`r_running`, `r_start`/`r_complete`/next-run) into `crates/engine/src/cmd/gp_r.rs` behind one `cfg` | removes ~20 scattered `cfg(feature = "r")` items |
| D7 | Workers Haiku; escalate to Sonnet after one failed revision, then the orchestrator | cost |
| D8 | Wave gate: `cargo xtask ci`; `cargo test -p rustgis-r -p rustgis-engine --features r` with Rscript+jsonlite+sf; `cargo build --target wasm32-unknown-unknown -p rustgis-web` | catch native R and wasm regressions locally |
| D9 | Keep the RFC with an erratum; trim the user guide to one page; keep only `MASTER_PLAN.md` + `*_10.md` in `planning/` | lean PR |

## Hazards (from recon)
- **H1** Twelve commits mix `planning/` and code (b38636a, 4e316ad, 044f35d, a922e17, 6da8579,
  8db6949, de98ba0, 05973a5, a2d9c34, a2ba210, ecc8c87, 7c710b9). Cherry-pick these with `-n`,
  restore `planning/` from HEAD, then `git commit -C <sha>`.
- **H2** `crates/engine/src/cmd/gp.rs` (959 lines) and `crates/engine/src/lib.rs` are shared by
  D3, D4 and D6, so those tasks are in separate waves. Enter both files by anchor only.
- **H3** `r_complete` is called from `crates/ui-egui/src/panes.rs:881` through
  `external_complete`, **outside** `Session::run`'s panic guard. The gp.rs comment says so
  ("Reading the host's output parses outside `Session::run`'s guard"). The reviewer's "redundant
  catch" finding is likely wrong. Task 10.4.2 moves the guard rather than deleting it.
- **H4** The web app enables `features = ["r"]` (apps/rustgis-web/Cargo.toml:13,15). With the shim
  gone, `gp.runR` on the web must fail cleanly through the `None` hook.
- **H5** Catalogs: if notice English text stays byte-identical, no `.tsv` change is needed.

## Wave table
| Wave | Theme | Tasks (parallel, disjoint files) |
|---|---|---|
| 1 | Branch and baseline (orchestrator only) | 10.1.1 |
| 2 | Independent trims | 10.2.1 web shim · 10.2.2 docs · 10.2.3 lang param |
| 3 | Typed notices | 10.3.1 |
| 4 | Engine tidy | 10.4.1 gp_r submodule · 10.4.2 panic guard · 10.4.3 hostile coverage |
| 5 | Final gate and PR prep (orchestrator only) | 10.5.1 |

Dependencies: 1 → 2 → 3 → 4 → 5. Within a wave, files are pairwise disjoint (checked below).

---

## 10.1.1 — Create the core branch (orchestrator)
- **Repo:** rustgis. **Depends:** none.
- **Steps:**
  1. `git switch -c feature/engine-r-core main`.
  2. For each sha in `git log --reverse --format=%h main..59f5ca3`: skip it if it only touches
     `planning/`. Otherwise run `git cherry-pick -n <sha>`, then
     `git restore --source=HEAD --staged --worktree -- planning` and `git clean -fd planning`,
     then `git commit -C <sha>` (H1).
  3. Copy `planning/MASTER_PLAN.md` and `planning/{EXECUTION_PLAN,PROMPT_TEMPLATE,STATE}_10.md`
     from `feature/engine-r-runtime`; commit as `docs(planning): add the R core PR plan`.
  4. Record the baseline in STATE_10.md: `cargo xtask ci`, the R test command and the wasm build
     (D8).
- **Acceptance:** `git diff 59f5ca3 HEAD --stat -- . ':!planning'` is empty; baseline recorded.

## 10.2.1 — Drop the webR shim, keep the hook
- **Files in scope:**
  - must edit: `crates/r/src/lib.rs`, `apps/rustgis-web/src/main.rs`, `apps/rustgis-web/src/web.rs`;
  - delete: `crates/r/src/web.rs`, `apps/rustgis-web/src/r.rs`;
  - may edit: `crates/r/src/tests.rs` (only `default_runner` cases).
- **Depends:** 10.1.1.
- **Spec:**
  - Remove `mod web`, `pub use web::WebRRunner` and the wasm arm of `default_runner`. Make
    `default_runner` (and its callers' use of it) native-only with
    `#[cfg(not(target_arch = "wasm32"))]`, matching how `RscriptRunner` is gated. Keep the
    `RScriptRunner` trait.
  - In the web app, delete `mod r;` and set `r_run: None` at `web.rs:608`. The `Services::r_run`
    hook in ui-egui stays unchanged.
  - Check that the engine's wasm path for `gp.runR` already fails with a clear error when there
    is no runner. If it does not, STOP and report (the engine is out of scope).
- **Anchors:** `crates/r/src/lib.rs:39` `pub use web::WebRRunner`, `:82` `fn default_runner`;
  `apps/rustgis-web/src/web.rs:608` `r_run: Some`; `apps/rustgis-web/src/main.rs:21` `mod r;`.
- **Reference:** geolibre's native-only gating in `crates/geolibre/src/lib.rs`.
- **Acceptance:**
  - `cargo test -p rustgis-r`;
  - `cargo build --target wasm32-unknown-unknown -p rustgis-web`;
  - `cargo clippy -p rustgis-r --target wasm32-unknown-unknown -- -D warnings`.

## 10.2.2 — Trim the docs
- **Files in scope:**
  - must edit: `docs/user-guide/r.md`, `docs/rfcs/001-engine-r-runtime.md`.
- **Depends:** 10.1.1.
- **Spec:**
  - Trim the user guide to one page covering:
    - installing R, `jsonlite` and `sf`;
    - Locate Rscript;
    - running `gp.runR` (params `script`, `inputs`, `args`, `timeout_s`, `lang`);
    - what a script reads and writes;
    - that R is reserved to the user;
    - that R doesn't run in the browser yet.
  - Drop P4 UI and P5 tool material.
  - Add an "Erratum" section to the RFC:
    - Rscript runs with `--no-save --no-restore --no-init-file`, not `--vanilla` (D9 of
      MASTER_PLAN);
    - there is no `catalog.rs`, so commands register in `cmd/gp.rs`;
    - native only for now; the webR bridge is future work behind `Services::r_run`.
- **Anchors:** `grep -n '^#' docs/user-guide/r.md`; `grep -n 'vanilla\|catalog.rs' docs/rfcs/001-engine-r-runtime.md`.
- **Acceptance:** `docs/user-guide/r.md` ≤ 80 lines; `grep -c Erratum docs/rfcs/001-engine-r-runtime.md` ≥ 1; `cargo xtask ci` unaffected (docs only).

## 10.2.3 — Language as a `gp.runR` param
- **Files in scope:**
  - must edit: `crates/engine/src/lib.rs`, `crates/engine/src/cmd/gp.rs`,
    `crates/engine/src/tests.rs`, `crates/ui-egui/src/lib.rs`.
- **Depends:** 10.1.1.
- **Spec:**
  - Remove `Session.lang` (field and initialiser).
  - Add an optional `lang` string param to `gp.runR`'s params struct and its `CommandSpec`
    param list.
  - Pass it to `RExecutionInput.lang`. `clean_lang` already sanitises it.
  - In ui-egui, delete the sync at `lib.rs:2112`. Where the app dispatches a command by id
    (find the single `fn run` / dispatch path), add `"lang": self.shown_lang.code()` to the
    params of `gp.runR` when the caller omitted it.
  - Update the engine test at `tests.rs:833` to pass `lang` in params.
  - `hostile_params_never_panic` must stay green.
- **Anchors:** `lib.rs:264-266` `pub lang: String`, `:402`; `gp.rs:147` `lang: s.lang.clone()`;
  `gp.rs:47` `"gp.runR"` registration; `ui-egui/src/lib.rs:2112`.
- **Do not touch:** queue code in gp.rs (10.4.1), notices (10.3.1).
- **Acceptance:**
  - `cargo test -p rustgis-engine --features r`;
  - `cargo test -p rustgis-ui-egui --features r`;
  - `grep -rn 'session.lang\|s\.lang' crates/` returns nothing.

## 10.3.1 — Typed R notices
- **Files in scope:**
  - must edit:
    - R crate: `crates/r/src/notices.rs`, `crates/r/src/lib.rs`, `crates/r/src/manifest.rs`,
      `crates/r/src/bootstrap.R`;
    - engine: `crates/engine/src/lib.rs` (`PendingRun`), `crates/engine/src/cmd/gp.rs`
      (`r_complete` only);
    - UI: `crates/ui-egui/src/rnotices.rs`, `crates/ui-egui/src/chrome.rs` (two call sites);
    - tests: `crates/r/src/manifest_tests.rs`, `crates/r/src/harness_tests.rs`.
  - delete: `crates/r/src/harness.rs` (fold its `include_str!` into `lib.rs`, D5).
- **Depends:** 10.2.1 (`crates/r/src/lib.rs`), 10.2.3 (`engine/lib.rs`, `gp.rs`).
- **Spec:**
  - Replace the string constants with
    `pub enum RNotice { NotFound, NeedJsonlite, NeedSf, NeedTerra }`. Give it
    `fn code(&self) -> &'static str` (`"need_sf"` …), `fn english(&self) -> &'static str` (keep
    today's text byte-identical, H5) and `fn from_code(&str) -> Option<Self>`.
  - bootstrap.R: `.rgis_need` and the jsonlite path write the code to a fixed channel. Use the
    manifest's error object if one exists; for jsonlite-missing (no manifest possible), use a
    stderr line `RUSTGIS_NOTICE:<code>`. `manifest.rs` / `native.rs` parse it into
    `RRunnerError::Notice(RNotice)`, whose `Display` is the English text.
  - Carry `Option<RNotice>` from `r_complete` into `PendingRun` and through to the UI notice
    (follow the existing path to `chrome.rs:211/545`).
  - `rnotices::translate` becomes `fn text(n: RNotice) -> String { match n { … => tl!("…") } }`.
  - Delete `R_FAILURE_PREFIX` stripping if it is no longer needed.
  - Rewrite the `notices.rs` sync test to check that every `code()` appears in bootstrap.R.
- **Anchors:** `notices.rs` (39 lines); `lib.rs:62` `enum RRunnerError`;
  `bootstrap.R:166,179,188,231`; `gp.rs:182` `R_FAILURE_PREFIX`; `chrome.rs:211,545`;
  `grep -n 'struct PendingRun' crates/engine/src/lib.rs`.
- **Reference:** `RRunnerError::NotFound` already carries a typed variant; extend that pattern.
- **Acceptance:**
  - `cargo test -p rustgis-r` and `cargo test -p rustgis-engine --features r` (with Rscript);
  - `cargo test -p rustgis-ui-egui --features r`;
  - `grep -rn 'strip_prefix(R_FAILURE_PREFIX)' crates/` returns nothing;
  - no `.tsv` diff.

## 10.4.1 — Collect the R queue in one submodule
- **Files in scope:**
  - must edit: `crates/engine/src/cmd/gp.rs`, `crates/engine/src/cmd/mod.rs`,
    `crates/engine/src/lib.rs` (`r_running` field only);
  - can create: `crates/engine/src/cmd/gp_r.rs`.
- **Depends:** 10.3.1.
- **Spec:**
  - Behaviour-preserving move. Put `run_r`, the R params struct, `r_start`/`r_complete`, the
    next-queued-run logic, `RRun`/`RRunStart` and `DROPPED_R_RUN` into `gp_r.rs`, gated once by
    `#[cfg(feature = "r")]` at the `mod` line.
  - `r_running` is read and written only inside `gp_r.rs` (test asserts in `tests.rs` may still
    read it, since the field stays `pub(crate)`). Expose
    `fn r_reset(s: &mut Session)` for `lib.rs:643`.
  - gp.rs re-exports what external callers use (`pub use gp_r::{RRun, …}`), so paths in ui-egui
    and the tests do not change.
  - Shared `ExternalJob` / `started` code stays in gp.rs.
- **Anchors:** `grep -n 'feature = "r"' crates/engine/src/cmd/gp.rs` (about 20 hits: lines
  43–478); `lib.rs:334,439,643`.
- **Do not touch:** `crates/engine/src/tests.rs`, ui-egui.
- **Acceptance:**
  - `cargo test -p rustgis-engine --features r`;
  - `cargo test -p rustgis-engine --features geolibre,r`;
  - `cargo test -p rustgis-engine` (no features);
  - `grep -c 'feature = "r"' crates/engine/src/cmd/gp.rs` ≤ 5.

## 10.4.2 — Move the manifest panic guard to the call site
- **Files in scope:**
  - must edit: `crates/r/src/manifest.rs`, `crates/ui-egui/src/panes.rs` (the
    `external_complete` call at ~881 only).
- **Depends:** 10.3.1.
- **Spec (H3):**
  - Remove `catch_unwind` from `convert_output`.
  - Because `r_complete` runs outside `Session::run`, wrap the `external_complete` call in
    `panes.rs` with the same last-resort guard `Session::run` uses. Find and reuse its helper;
    do not write a new one.
  - If no reusable helper exists, STOP and report: keeping the inner guard with a comment
    pointing to H3 is the fallback.
- **Anchors:** `manifest.rs:11,22` `catch_unwind`; `panes.rs:881`;
  `grep -n catch_unwind crates/engine/src/lib.rs`.
- **Acceptance:** `cargo test -p rustgis-r`; `cargo test -p rustgis-ui-egui --features r`;
  `cargo clippy --workspace -- -D warnings`.

## 10.4.3 — Hostile-params coverage with R on
- **Files in scope:**
  - must edit: `crates/engine/src/tests.rs`.
- **Depends:** 10.3.1.
- **Spec:**
  - Confirm `hostile_params_never_panic` (`tests.rs:577`) iterates `gp.runR` and `gp.locateR*`
    when built with `--features r`, and that neither is in its skip list without reason.
  - Add junk that targets R: `{"script": 7}`, `{"script": "x", "inputs": [null, 1e30]}`,
    `{"timeout_s": -1}`, `{"lang": "../../"}`.
  - Add a regression test that `gp.runR` with no runner configured returns `Err`, not a panic.
- **Acceptance:** `cargo test -p rustgis-engine --features r hostile`; `cargo test -p rustgis-engine hostile`.

Wave 4 file disjointness: 10.4.1 {gp.rs, cmd/mod.rs, gp_r.rs, engine lib.rs} · 10.4.2
{manifest.rs, panes.rs} · 10.4.3 {engine tests.rs}. ✓
Wave 2: 10.2.1 {r/lib.rs, r/web.rs, r/tests.rs, web app} · 10.2.2 {docs} · 10.2.3 {engine
lib.rs, gp.rs, engine tests.rs, ui-egui lib.rs}. ✓

## 10.5.1 — Final gate and PR prep (orchestrator)
- Run the full D8 gate serially.
- Delete from `planning/` everything except `MASTER_PLAN.md` and `*_10.md`; `tasks/10.*` may stay
  until merge, then go.
- `git diff main...HEAD --stat` should show roughly 4k lines.
- Draft the PR body: what lands, what is deferred (the Out of scope list), and how to test.
  Push and open the PR only on the user's go-ahead.
