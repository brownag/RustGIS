# State — Orchestration 10 (R core PR)

Branch: `feature/engine-r-core` (to be created by 10.1.1 from `main` = be44b0b)
Source: `feature/engine-r-runtime` commits `ac02a1d..7df1a29` (tip `59f5ca3`), planning stripped.

## Baseline (fill in during 10.1.1)
- [ ] `cargo xtask ci` —
- [ ] `cargo test -p rustgis-r -p rustgis-engine --features r` (Rscript + jsonlite + sf) —
- [ ] `cargo build --target wasm32-unknown-unknown -p rustgis-web` —
- [ ] `git diff 59f5ca3 HEAD --stat -- . ':!planning'` empty —

## Wave board
| Task | Wave | Status |
|---|---|---|
| 10.1.1 branch + baseline | 1 | pending |
| 10.2.1 drop webR shim, keep hook | 2 | pending |
| 10.2.2 trim docs + RFC erratum | 2 | pending |
| 10.2.3 lang as gp.runR param | 2 | pending |
| 10.3.1 typed R notices (+ fold harness.rs) | 3 | pending |
| 10.4.1 gp_r submodule | 4 | pending |
| 10.4.2 move manifest panic guard | 4 | pending |
| 10.4.3 hostile coverage with R | 4 | pending |
| 10.5.1 final gate + PR prep | 5 | pending |

## Task reports
