# RFC 001: Engine-Level R Runtime and Geoprocessing Subsystem (docs/rfcs/001-engine-r-runtime.md)

> **Erratum (cleaned copy).** This copy was cleaned mechanically from the raw RFC. Inline images are dropped or replaced: `→` for the arrows, `2×10⁹` and `4×10¹⁰` for the fuel budgets, and `[figure omitted]` for the rest. Markdown escapes are removed and headings are no longer bold. Code blocks that the raw file left unfenced are wrapped in plain code fences. Body text and citations are otherwise verbatim. Corrections that bind the phases are in [MASTER_PLAN](https://github.com/opengeos/rustgis/blob/main/planning/MASTER_PLAN.md), sections "Names (RFC → repo)" and "RFC corrections that bind the phases". The fact base is [r-runtime-findings](https://github.com/opengeos/rustgis/blob/main/planning/research/r-runtime-findings.md).

## Executive Summary and Architectural Context

RustGIS is engineered as a fast, local-first, cross-platform Geographic Information System written in pure Rust that compiles natively for Windows, macOS, and Linux, executes in-process within Python and Jupyter notebooks, and compiles directly to WebAssembly (wasm32-unknown-unknown) for zero-install execution in the browser at web.rustgis.app1. The system architecture is governed by four foundational engineering invariants: strict crate layering enforced by cargo xtask layers, universal command dispatch where every user interface or programmatic action maps to a declarative command identifier, absolute memory and runtime safety under #![forbid(unsafe_code)] with zero unwrap(), expect(), or panic!() calls in non-test code, and unbroken WebAssembly compilation across all crates situated below the apps/ directory2.  
While the core geoprocessing layer (rustgis-analysis, built atop geo = "0.33") handles fundamental vector and raster operations in pure Rust, and rustgis-geolibre integrates over 1,000 terrain, hydrology, LiDAR, and remote sensing tools from the GeoLibre and Whitebox suites, advanced spatial statistics—such as spatial econometrics, point pattern analysis, geostatistics, network topology, and hierarchical spatial modeling—rely heavily on the R statistical ecosystem (sf, wk, nanoarrow, geoarrow, igraph, and lme4)1. Integrating an engine-level R execution subsystem requires bridging a dynamically typed, C/Fortran-backed statistical runtime into RustGIS without violating the workspace's unsafe_code = "forbid" constraint, crashing the host process on malformed inputs, or inflating the WebAssembly binary beyond browser memory limits2.  
This specification defines the architecture for rustgis-r (crates/r-runner), a Layer 3 (L3) subsystem that mirrors the dual-target asynchronous execution pattern established by rustgis-geolibre2. On desktop, CLI, and Python hosts (apps/rustgis, apps/rustgis-cli, apps/rustgis-python), the subsystem executes an isolated Rscript child process communicating over an out-of-process binary staging protocol1. In the browser (apps/rustgis-web), the subsystem delegates execution through the engine's Services abstraction to an asynchronous WebR Web Worker instance (external_start → Services::r_run → gp.finish), exchanging vector and raster layers via zero-base64 FlatGeobuf/WKB, GeoTIFF, and nanoarrow buffers while preserving atomic undo/redo transaction semantics2.

## Crate Topology and Layering Invariants

The RustGIS Cargo workspace (resolver = "3", edition = "2024", rust-version = "1.90") organizes its internal crates into seven strict architectural tiers (L0 through L6) topped by platform-specific application binaries in apps/*2. The workspace verification tool (cargo xtask layers, executed automatically during cargo xtask ci) enforces a directed acyclic dependency graph in which each crate may only depend on crates situated in lower layers, and no crate below Layer 6 (rustgis-ui-egui) may depend on user interface frameworks (egui, eframe, or rfd)1. Furthermore, every crate below apps/ must compile cleanly for wasm32-unknown-unknown, requiring web-time = "1" in place of std::time and routing all external file or process I/O through host-provided service traits2.  
[figure omitted]  
To preserve complete architectural symmetry with rustgis-geolibre, the R execution subsystem is introduced as a new Layer 3 crate at crates/r-runner (package name rustgis-r) and exposed to rustgis-engine (L4) behind a Cargo feature flag named r-runtime, which is enabled by default alongside geolibre2. Placing rustgis-r at Layer 3 allows it to depend directly on rustgis-geom (L0), rustgis-data (L1), and rustgis-map and rustgis-analysis (L2) to serialize vector layers, attributes, active feature selections, and rasters without creating circular dependencies with rustgis-engine (L4)1.

| Architectural Tier | Workspace Crate (path) | Core Responsibilities & Invariants | R Subsystem Modifications (RFC 001) |
| :---- | :---- | :---- | :---- |
| **L0** | rustgis-geom (crates/geom) | Coordinates, bounding extents, Web Mercator projection, geodesic distance/area, scale1. | Unchanged; provides CRS/extent metadata for R spatial objects2. |
| **L1** | rustgis-data (crates/data) | Features, typed attributes, SQL WHERE clauses, GeoJSON/CSV/Shapefile I/O, in-place providers (flatgeobuf = "6", parquet = "60", rusqlite = "0.40"), rasters (tiff = "0.11", proj4rs = "0.2")2. | Exposes zero-copy WKB/FlatGeobuf and GeoTIFF buffer writers for rustgis-r payload staging2. |
| **L2** | rustgis-map (crates/map), rustgis-analysis (crates/analysis), rustgis-tiles (crates/tiles) | Map layers, symbology, labels, camera, web tiles (XYZ/TMS/WMS/MVT), native geo = "0.33" geoprocessing tools and ToolSpec definitions1. | Registers r:* tool namespace descriptors and script parameter schemas in the analysis tool registry2. |
| **L3** | rustgis-render (crates/render), rustgis-plugins (crates/plugins), rustgis-geolibre (crates/geolibre), **rustgis-r (crates/r-runner)** | Offscreen tiny-skia renderer; wasmi sandboxed plugins; GeoLibre/Whitebox runner; **New:** R tool catalog, script AST/parameter sanitizer, IPC envelope builder, and native Rscript process spawner2. | New crate crates/r-runner (#![forbid(unsafe_code)]); compiles to both native targets and wasm32-unknown-unknown2. |
| **L4** | rustgis-engine (crates/engine) | Session state, undo stack, CommandSpec registry (src/cmd/*.rs), feature parity checklist (src/catalog.rs), Services host trait2. | Adds Services::r_run, r.runScript command, gp.run dispatch for r:* tools, and gp.finish reconciliation2. |
| **L5** | rustgis-assistant (crates/assistant) | Sans-I/O AI chat agent (OpenAI, Anthropic, Gemini, OpenRouter) translating user prompts into engine commands with user approval gates2. | Classifies r.runScript and gp.run (tool: "r:*") under script approval gates alongside file/network/plugin operations2. |
| **L6** | rustgis-ui-egui (crates/ui-egui) | Ribbon UI, Tools pane, run history, ModelBuilder, attribute table, 16-language i18n catalogs (src/i18n/*.tsv), headless ui_shot1. | Adds R Script Editor pane, Tools gallery R Spatial category, and localized strings via tl!("...") across all *.tsv files1. |
| **Apps** | apps/rustgis, apps/rustgis-cli, apps/rustgis-web, apps/rustgis-python | Desktop GUI (eframe), headless CLI & MCP server, browser Wasm app (trunk), and Python bindings (pip install rustgis)2. | Implements Services::r_run natively via std::process::Command (Rscript) and in rustgis-web via WebR Worker IPC2. |

### Workspace Configuration and Lint Enforcement

Every module inside crates/r-runner inherits the workspace lints from the root Cargo.toml (unsafe_code = "forbid", dbg_macro = "warn", todo = "warn") and includes the crate-level denial block mandated by AGENTS.md (#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)])2. Because crates/r-runner is matched by members = ["crates/*", "apps/*", "xtask"], continuous integration (cargo xtask ci) automatically compiles it for wasm32-unknown-unknown when building rustgis-web1. Platform-specific process execution logic inside crates/r-runner is conditionally compiled using #[cfg(not(target_arch = "wasm32"))], whereas the wasm32 target compiles only the pure-Rust tool specifications, input and output buffer serializers, and job request structures2.

## Engine Extension Layer and Asynchronous Execution Lifecycle

### Command Dispatch and Extension Patterns in crates/engine

In rustgis-engine (L4), every state mutation or analysis operation is encapsulated as a CommandSpec in crates/engine/src/cmd/*.rs and tracked against the ribbon feature parity catalog in crates/engine/src/catalog.rs, which is verified by rustgis-cli parity against a monotonically rising coverage floor2. Commands never open UI dialogs directly; instead, UI-only commands (ui.*) reside in crates/ui-egui/src/lib.rs, while engine commands are invoked uniformly across the ribbon UI, keyboard shortcuts, command search (Ctrl+K), rustgis-cli run --cmd, the Python Session.run() API, and the AI Assistant using a command identifier and JSON parameter payload—such as gp.run={"tool":"buffer","params":{"input":"Cities","distance":100}}1.  
RustGIS currently supports three distinct execution mechanisms for geoprocessing tools dispatched through gp.run2. Built-in native Rust tools in rustgis-analysis (L2) run synchronously in-memory over geo = "0.33" primitives and rustgis-data layers1. Sandboxed WebAssembly plugins in rustgis-plugins (L3) are packaged as .rgplugin archives containing a plugin.json manifest and a wasm32 module executed synchronously in-process by the wasmi interpreter2. Plugins communicate through three host imports in module rustgis (call, result, and log) and two module exports (rustgis_alloc and rustgis_call), operating under a deterministic instruction fuel budget of 2×10⁹ instructions for a command and 4×10¹⁰ instructions for a tool8. Each plugin tool receives layer features as GeoJSON (filtered to only the selected features when a layer has an active selection) and returns a ToolOutput JSON object containing layers, tables, selection, and messages8. In contrast, heavy external tool suites like GeoLibre and Whitebox (rustgis-geolibre at L3, providing 1,000+ tools) cannot execute synchronously inside wasmi on the main UI thread without freezing the interface or exhausting synchronous instruction fuel budgets1. Consequently, rustgis-geolibre utilizes a split-phase asynchronous lifecycle: external_start → Services::geolibre_run → gp.finish2.  
Because R scripts load complex statistical libraries, execute C and Fortran linear algebra routines, and require hundreds of milliseconds to several seconds in WebR or native Rscript, the R subsystem must mirror the split-phase rustgis-geolibre execution pattern (external_start → Services::r_run → gp.finish) rather than the synchronous wasmi plugin model2.  
[figure omitted]

### Five-Stage Lifecycle of an R Tool Invocation

The end-to-end execution of an R tool or user script progresses through five deterministic stages across rustgis-r (L3), rustgis-engine (L4), and the host application (apps/*)2. During the first stage (Registration and Discovery), rustgis-r populates the geoprocessing tool catalog at engine startup with built-in R spatial tools prefixed with r: (such as r:kriging, r:local-moran, r:point-density, and r:spatial-regression) and registers the general-purpose r.runScript command in crates/engine/src/cmd/r_script.rs2. Because each tool entry declares typed parameters (input vector or raster layers, attribute field names, numeric bandwidths, and formula strings), rustgis-cli tools, the GUI Tools pane, ModelBuilder, and the MCP server's search_tools and describe_tool endpoints inspect R tools identically to native or GeoLibre tools1.  
During the second stage (Command Dispatch and Hostile Input Validation), a user or script invokes gp.run={"tool":"r:local-moran","params":{"polygons":"Countries","field":"gdp_md"}} or r.runScript={"script":"...","inputs":["Cities"]} through Session::run1. In accordance with AGENTS.md, Session::run catches any escaped panics as a last resort and validates all command parameters as hostile input—using .get() rather than slice indexing, clamping numeric bounds, bounding recursion, and rejecting path traversal tokens2. If an input layer is backed by a large file read in-place via a lazy data provider (GeoPackage, GeoParquet, or FlatGeobuf) where only the features of the last view are held in memory, the engine ensures full feature availability (equivalent to layer.loadAll) or extracts only the currently selected features when the layer has an active selection3.  
During the third stage (Staging via external_start), crates/engine/src/cmd/gp.rs invokes external_start, allocating a monotonically increasing job_id: u64 in the session's Geoprocessing run history with status Running1. The rustgis-r serializer encodes each requested input vector layer into an in-memory binary buffer (FlatGeobuf with WKB geometries via flatgeobuf = "6" and geozero = "0.15", or GeoJSON for small layers) and each input raster layer into a GeoTIFF byte array (tiff = "0.11"), bundling them with the R bootstrap script and interface language code (lang, supporting RustGIS's 16 languages) into an RJobRequest structure1.  
During the fourth stage (Host Service Execution via Services::r_run), external_start passes the RJobRequest to services.r_run(request) on the host Services trait2. In rustgis-cli (the headless synchronous host), Services::r_run blocks on the Rscript subprocess, captures standard output and standard error lines, reads the output manifest, and immediately dispatches session.run("gp.finish", ...) before returning to the CLI caller so sequential --cmd flags (such as share.exportMap) observe the newly created layers2. In apps/rustgis (Desktop GUI) and apps/rustgis-web (Browser Wasm), Services::r_run returns immediately so the egui render loop never stalls2. The desktop host spawns Rscript on a background OS worker thread, whereas the browser host transfers the RJobRequest buffers to the WebR Web Worker2. Streaming stdout and stderr lines emitted during execution are forwarded to the Geoprocessing history pane as progress events1.  
During the fifth stage (Reconciliation and Atomic Undo Commit via gp.finish), the host dispatches the internal command gp.finish carrying the job_id and serialized RJobResponse once the R runtime completes2. The gp.finish handler parses the returned vector/raster layers, standalone tables, feature selection indices, and console log messages (notices), inserts the new layers into the Session map (layersAdded), and commits the entire mutation as a single undo step on the session undo stack8. If the R execution fails, gp.finish records the job as failed in the run history, surfaces the R error condition in notices, and leaves the project layers and undo stack untouched8.

### Security and Privilege Boundaries Across Hosts

RustGIS enforces strict privilege separation across its execution hosts to prevent unauthorized script execution or sandbox escapes2. In the Model Context Protocol server (rustgis-cli mcp), docs/mcp.md explicitly stipulates that plugin management, user scripts, tasks, models, and the GeoLibre runner are never executed from the MCP server11. Consequently, Services::r_run in the MCP server host implementation unconditionally rejects execution with Err(EngineError::ForbiddenInMcp("R runtime and scripts are never run from the MCP server"))11. In the interactive AI Assistant (rustgis-assistant at L5), the sans-I/O chat agent builds and reads OpenAI, Anthropic, Gemini, and OpenRouter requests and loops over tool calls to execute commands, but file, network, and plugin commands must wait for the user's explicit approval2. Both r.runScript and gp.run targeting r:* tools are therefore classified inside crates/assistant as privileged script operations requiring interactive user approval before execution2. Finally, inside the WebAssembly plugin sandbox (rustgis-plugins at L3), docs/plugins.md mandates that no plugin may run the user's scripts, tasks, or models, or install or locate external software8. The host request dispatcher in crates/plugins enforces this boundary by blocking any plugin call request targeting r.runScript or gp.run with an r: tool prefix, even when the plugin possesses the edit or files permission8.

### Core Rust Trait and Struct Specifications

The Layer 3 crate crates/r-runner/src/lib.rs defines the platform-agnostic wire structures under #![forbid(unsafe_code)] and strict Clippy lints2:

Rust  

```
// crates/r-runner/src/lib.rs  
#![forbid(unsafe_code)]  
#![deny(  
    clippy::unwrap_used,  
    clippy::expect_used,  
    clippy::panic,  
    clippy::indexing_slicing,  
    clippy::todo,  
    clippy::unreachable  
)]
```

use serde::{Deserialize, Serialize};

```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]  
pub enum RWireFormat {  
    FlatGeobuf,  
    GeoJson,  
    GeoTiff,  
    ArrowIpc,  
}
```

```
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct RInputArtifact {  
    pub param_name: String,  
    pub layer_name: String,  
    pub format: RWireFormat,  
    pub crs_epsg: Option<u32>,  
    pub data: Vec<u8>,  
}
```

```
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct RJobRequest {  
    pub job_id: u64,  
    pub tool_id: String,  
    pub r_script: String,  
    pub params_json: serde_json::Value,  
    pub inputs: Vec<RInputArtifact>,  
    pub lang: String,  
    pub timeout_ms: u64,  
}
```

```
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct ROutputLayer {  
    pub name: String,  
    pub format: RWireFormat,  
    pub data: Vec<u8>,  
}
```

```
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct RSelectionOutput {  
    pub layer: String,  
    pub rows: Vec<u64>,  
}
```

```
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct RJobResponse {  
    pub job_id: u64,  
    pub ok: bool,  
    pub layers: Vec<ROutputLayer>,  
    pub tables: Vec<serde_json::Value>,  
    pub selection: Option<RSelectionOutput>,  
    pub messages: Vec<String>,  
    pub error: Option<String>,  
}
```

In crates/engine/src/services.rs, the host Services trait is extended alongside geolibre_run so that every application target implements the R execution hook2:

Rust  
// crates/engine/src/services.rs  
pub trait Services {  
    // Existing host service hooks (file I/O, network, geolibre_run)...  
    fn geolibre_run(&mut self, job: GeolibreJobRequest) -> Result<(), ServiceError>;

    /// Dispatches an R geoprocessing tool or script to the platform runner.  
    /// Native CLI blocks and calls `gp.finish` inline; Desktop GUI and WebR  
    /// execute asynchronously and invoke `gp.finish` upon completion.  
    fn r_run(&mut self, job: rustgis_r::RJobRequest) -> Result<(), ServiceError>;  
}

## Browser Execution Path via WebR in rustgis-web

### Worker Instantiation: Dedicated Web Worker vs. SharedWorker

The browser application (apps/rustgis-web, built with trunk and targeting wasm32-unknown-unknown) runs the rustgis-ui-egui interface and tiny-skia renderer inside the browser's main UI thread2. WebR is a port of the R interpreter compiled from C and Fortran via Emscripten to WebAssembly and engineered to execute inside a separate JavaScript Web Worker thread so that long-running statistical computations never block the main browser event loop7.  
When architecting the worker topology in apps/rustgis-web, a Dedicated Web Worker instantiated via WebR's new WebR(options) constructor and awaited via await webR.init() is strictly superior to a SharedWorker7. The Dedicated Web Worker binds 1:1 with the open web.rustgis.app tab or Jupyter rustgis.Map widget iframe1. While a SharedWorker could theoretically share a single WebR runtime across multiple browser tabs, R's internal C runtime is single-threaded with mutable global state (.GlobalEnv, active graphics devices, random number generator seeds, and temporary working directories)7. Concurrent gp.run requests from two open RustGIS tabs would collide in .GlobalEnv or block one another, and SharedWorker lacks universal support across mobile browsers and complicates transferring ImageBitmap canvas plots15. Furthermore, apps/rustgis-web instantiates the Dedicated Web Worker lazily on the first Services::r_run invocation (or when the user opens the R Script pane) rather than at initial page load, preserving RustGIS's rapid startup and low memory footprint for sessions that only perform map viewing, styling, or native Rust geoprocessing1.

### Cross-Origin Isolation Headers vs. Basemap Tile Fetching

A critical systems engineering conflict arises in apps/rustgis-web regarding WebR's inter-thread communication channel7. WebR supports two transport channels between the main browser thread and its Emscripten Web Worker: a SharedArrayBuffer channel and a PostMessage channel (ChannelType.PostMessage)7. The SharedArrayBuffer channel enables synchronous shared-memory coordination between threads, but browsers only expose SharedArrayBuffer when window.crossOriginIsolated evaluates to true, which requires serving the top-level document with both Cross-Origin-Opener-Policy: same-origin (COOP) and Cross-Origin-Embedder-Policy: require-corp (COEP) HTTP headers7. When those headers are absent, WebR automatically falls back to using its PostMessage communication channel, or the PostMessage channel can be explicitly selected via WebROptions.channelType: ChannelType.PostMessage16.  
In RustGIS, enforcing Cross-Origin-Embedder-Policy: require-corp on web.rustgis.app would break core GIS workflows in rustgis-tiles (L2) and rustgis-python1. RustGIS allows users to dynamically load arbitrary third-party XYZ, TMS, and WMS raster tiles, Mapbox Vector Tiles, OGC API Features, WFS, and ArcGIS REST services, and embeds web.rustgis.app inside Jupyter notebooks via rustgis.Map, where the project and its data travel between Python and the browser app over postMessage1. Under COEP: require-corp, the browser blocks any cross-origin tile or WFS HTTP response that lacks an explicit Cross-Origin-Resource-Policy: cross-origin header, breaking OpenStreetMap tiles, public government WMS servers, and cross-origin Jupyter notebook iframe embedding3.  
Because RustGIS dispatches R tools as non-interactive batch jobs (external_start → Services::r_run → gp.finish) using shelter.captureR() and webR.evalR(), the R session is automatically placed into non-interactive mode during evaluation2. Consequently, synchronous SharedArrayBuffer blocking for interactive readline() prompts is unnecessary for geoprocessing tools15. By configuring apps/rustgis-web to use ChannelType.PostMessage (or ChannelType.Automatic, which seamlessly falls back to PostMessage when crossOriginIsolated is false) and transferring binary Uint8Array layer buffers into Emscripten's virtual filesystem via webR.FS.writeFile() and webR.FS.readFile(), rustgis-web achieves high-throughput execution while requiring zero COOP or COEP headers and maintaining 100% compatibility with external basemap tiles and Jupyter notebooks3.

### Virtual Filesystem Mounting and Spatial Package Pre-Loading

WebR executes atop Emscripten's virtual filesystem (FS), providing both in-memory storage (MEMFS) and persistent browser storage (IDBFS backed by IndexedDB or OPFS), as well as support for mounting pre-compiled WebR filesystem images hosted on static web servers and r-multiverse / r-universe5.

| Storage / Transport Component | WebR Mechanism | RustGIS Browser Usage Pattern | Persistence & Memory Characteristics |
| :---- | :---- | :---- | :---- |
| **Worker Transport** | ChannelType.PostMessage (default fallback) or SharedArrayBuffer7. | Configured with ChannelType.Automatic (resolving to PostMessage when crossOriginIsolated is false so external WMS/XYZ tiles and Jupyter rustgis.Map iframes never fail)3. | Zero impact on cross-origin tile fetching; supports full async captureR() and FS operations15. |
| **Job Staging (/tmp/rustgis/job_<id>)** | Emscripten MEMFS via webR.FS.mkdir(), webR.FS.writeFile(), webR.FS.readFile(), and unlink()17. | Ephemeral directory per job_id holding input FlatGeobuf/GeoTIFF byte arrays and output layers2. | Stored in worker Wasm linear memory; explicitly unlinked in a finally block after gp.finish to prevent Wasm heap exhaustion6. |
| **Spatial Package Cache (/usr/lib/R/site-library)** | Emscripten IDBFS + webR.FS.mount() synced to browser IndexedDB / OPFS5. | Caches pre-compiled WebAssembly R packages (wk, nanoarrow, geoarrow, sf, rlang, vctrs, cli) across browser reloads5. | Avoids re-downloading compiled WebR package archives on subsequent sessions5. |
| **Static VFS Package Bundles** | r-multiverse / r-universe WebR-compatible filesystem images5. | Mounted lazily when a specific R tool declares packages = ["sf", "nanoarrow"] in its ToolSpec5. | Pre-linked shared libraries (PROJ/GEOS/GDAL subsets in sf) mounted directly into VFS without runtime compilation5. |
| **Object Memory Management** | new webR.Shelter() and shelter.purge()7. | Wraps every shelter.captureR() invocation inside Services::r_run7. | Prevents R RObject proxy references from leaking inside the WebR worker across consecutive tool runs7. |

### Browser WebR Runner Implementation in apps/rustgis-web

In apps/rustgis-web, the Rust wasm32 module invokes an asynchronous JavaScript bridge via wasm-bindgen when Services::r_run is called. The bridge manages lazy initialization, MEMFS staging, Shelter lifecycle cleanup (shelter.purge()), and completion dispatch back into gp.finish2:

JavaScript  
// apps/rustgis-web/js/webr_bridge.js  
import { WebR, ChannelType } from 'https://webr.r-wasm.org/v0.5.4/webr.mjs';

let webRInstance = null;  
let initPromise = null;

async function ensureWebRInitialized() {  
  if (webRInstance) return webRInstance;  
  if (!initPromise) {  
    initPromise = (async () => {  
      const channelType = globalThis.crossOriginIsolated  
        ? ChannelType.Automatic  
        : ChannelType.PostMessage;  
      const webr = new WebR({ channelType });  
      await webr.init();  
      await webr.FS.mkdir('/tmp/rustgis');  
      webRInstance = webr;  
      return webr;  
    })();  
  }  
  return initPromise;  
}

export async function executeRJobInWorker(jobRequest, onCompleteCallback) {  
  const webr = await ensureWebRInitialized();  
  const shelter = await new webr.Shelter();  
  const jobDir = `/tmp/rustgis/job_${jobRequest.job_id}`;

  try {  
    await webr.FS.mkdir(jobDir);  
    await webr.FS.mkdir(`${jobDir}/in`);  
    await webr.FS.mkdir(`${jobDir}/out`);

    // Write binary input artifacts (FlatGeobuf / GeoTIFF / GeoJSON) directly into MEMFS  
    for (const artifact of jobRequest.inputs) {  
      const ext = artifact.format === 'FlatGeobuf' ? 'fgb'  
                : artifact.format === 'GeoTiff' ? 'tif' : 'geojson';  
      const filePath = `${jobDir}/in/${artifact.param_name}.${ext}`;  
      await webr.FS.writeFile(filePath, new Uint8Array(artifact.data));  
    }

    // Write job metadata and user/tool script  
    const metaBytes = new TextEncoder().encode(jobRequest.params_json);  
    await webr.FS.writeFile(`${jobDir}/params.json`, metaBytes);  
    const scriptBytes = new TextEncoder().encode(jobRequest.r_script);  
    await webr.FS.writeFile(`${jobDir}/script.R`, scriptBytes);

    // Execute via captureR in non-interactive mode, capturing stdout, stderr, and conditions  
    const capture = await shelter.captureR(  
      `source("${jobDir}/bootstrap.R", local = new.env(parent = globalenv()))`,  
      { withAutoprint: true, captureStreams: true, captureConditions: true }  
    );

    const messages = capture.output  
      .filter(evt => evt.type === 'stdout' || evt.type === 'stderr' || evt.type === 'message' || evt.type === 'warning')  
      .map(evt => `[${evt.type}] ${typeof evt.data === 'string' ? evt.data : ''}`);

    const manifestRaw = await webr.FS.readFile(`${jobDir}/out/manifest.json`);  
    const manifest = JSON.parse(new TextDecoder().decode(manifestRaw));

    const outputLayers = [];  
    for (const layerSpec of (manifest.layers || [])) {  
      const bytes = await webr.FS.readFile(`${jobDir}/out/${layerSpec.file}`);  
      outputLayers.push({  
        name: layerSpec.name,  
        format: layerSpec.format,  
        data: Array.from(bytes)  
      });  
    }

    onCompleteCallback(JSON.stringify({  
      job_id: jobRequest.job_id,  
      ok: true,  
      layers: outputLayers,  
      tables: manifest.tables || [],  
      selection: manifest.selection || null,  
      messages,  
      error: null  
    }));  
  } catch (err) {  
    onCompleteCallback(JSON.stringify({  
      job_id: jobRequest.job_id,  
      ok: false,  
      layers: [],  
      tables: [],  
      selection: null,  
      messages: [],  
      error: String(err && err.message ? err.message : err)  
    }));  
  } finally {  
    await shelter.purge();  
    // Clean up MEMFS staging files to prevent Wasm linear memory bloat  
    try {  
      await webr.evalRVoid(`unlink("${jobDir}", recursive = TRUE, force = TRUE)`);  
    } catch (_) {}  
  }  
}

## Desktop and CLI Native Execution Path

For native targets (apps/rustgis, apps/rustgis-cli, and apps/rustgis-python), two architectural candidates exist for executing R on Windows, macOS, and Linux: spawning an out-of-process Rscript child process over OS pipes and temporary staging files, or embedding the R C shared library (libR.so, libR.dylib, or R.dll) in-process via C-FFI bindings (extendr-api / libR-sys)1.

### Evaluation Against RustGIS Core Engineering Invariants

Evaluating both candidates against the repository's non-negotiable rules in Cargo.toml and AGENTS.md demonstrates that out-of-process Rscript execution is the only architecture compatible with RustGIS's safety, packaging, and layering guarantees2. First, the root Cargo.toml enforces [workspace.lints.rust] unsafe_code = "forbid" across the workspace2. Embedding libR in-process requires unsafe extern "C" FFI calls (Rf_initEmbeddedR, R_ParseEvalString, and SEXP pointer dereferencing). Even if encapsulated inside an external crate, initializing and calling libR introduces undefined behavior hazards whenever R's garbage collector (PROTECT/UNPROTECT stack) or C longjmp error unwinds cross Rust stack frames. Spawning Rscript via std::process::Command in crates/r-runner requires zero unsafe code, maintaining 100% compliance with workspace lints2.  
Second, AGENTS.md enforces the "Never crash" mandate, treating all file inputs and command parameters as hostile and verifying resilience via crates/engine/src/tests.rs::hostile_params_never_panic2. In-process libR violates this invariant because native R spatial packages (sf, terra, stars) wrap C and C++ libraries (GEOS, GDAL, PROJ) that can trigger SIGSEGV or SIGABRT on degenerate geometries or out-of-memory conditions, which would immediately crash the user's rustgis desktop window and lose unsaved project state. Moreover, libR installs process-wide POSIX signal handlers and stack-checking bounds that conflict with Rust worker threads and Python's interpreter when running inside rustgis-python (pip install rustgis)1. Because libR is single-threaded and cannot be safely re-initialized if terminated mid-computation, job cancellation is impossible in-process. With an Rscript child process, job cancellation or timeout enforcement deterministically invokes Child::kill(), and any C-level segmentation fault inside R is cleanly captured as a non-zero exit status and reported as a user-facing error notice via gp.finish2.  
Third, RustGIS's core distribution model is a single, small, GPU-drawn native binary with "no runtime to install and no server to run," distributed via winget install OpenGeos.RustGIS, brew install --cask opengeos/rustgis/rustgis, yay -S rustgis-bin (AUR), Fedora COPR (dnf install rustgis), Windows MSI/MSIX/setup.exe, signed and notarized macOS DMG, and Linux .deb, .rpm, and AppImage packages1. Linking libR at build time would either prevent rustgis from starting on machines without R installed or require bundling a multi-hundred-megabyte R distribution inside every installer. Mirroring how rustgis-geolibre runs the optional geolibre program natively when available on the host machine, the Rscript subprocess runner keeps the core RustGIS binary completely self-contained and dependency-free1.

| Evaluation Criterion | Out-of-Process Rscript Runner (Selected) | Embedded libR / extendr-api C-FFI (Rejected) |
| :---- | :---- | :---- |
| **unsafe_code = "forbid" Compliance** | **100% Pure Safe Rust** (std::process::Command, std::io::BufReader)2. | Violates unsafe_code = "forbid" via C-FFI bindings and SEXP pointer manipulation2. |
| **Crash & Segfault Isolation (Never Crash)** | **Complete OS Process Isolation:** C/Fortran segfaults or OOMs in R return a clean gp.finish error notice; host GUI never crashes2. | **Fatal Host Termination:** Any SIGSEGV, SIGABRT, or C longjmp across FFI boundaries crashes rustgis and loses session state2. |
| **Job Cancellation & Timeouts** | Instantaneous and deterministic via OS Child::kill() on timeout or user cancel. | Unsafe/impossible to preempt a blocked C/Fortran loop inside single-threaded libR. |
| **Desktop & Python Packaging** | **Zero build-time C dependencies;** works seamlessly with winget, brew, AppImage, and pip install rustgis1. | Requires libR headers in CI matrix and breaks rustgis-python due to conflicting C signal handlers1. |
| **Architectural Symmetry** | Identical to rustgis-geolibre native runner (invoking external geolibre binary)2. | Introduces a one-off FFI execution model alien to the rest of the workspace2. |
| **Invocation Latency Overhead** | ~35–80 ms process spawn (Rscript --vanilla), negligible for spatial statistics jobs. | ~1–5 ms function call after initial Rf_initEmbeddedR startup. |

### Native Runner Execution Mechanics in crates/r-runner

When Services::r_run executes on desktop (apps/rustgis), CLI (apps/rustgis-cli), or Python (apps/rustgis-python), crates/r-runner/src/native.rs locates the Rscript binary by probing $RUSTGIS_RSCRIPT_PATH, $R_HOME/bin/Rscript, and the system PATH (alongside standard OS installation paths on Windows, macOS, and Linux)1. If Rscript is not present, r_run returns a localized error notice via tl!("...") instructing the user how to install R or configure $RUSTGIS_RSCRIPT_PATH2.  
When Rscript is found, the runner creates an isolated temporary job directory containing in/ and out/ subdirectories, writes the binary input layers (in/<param_name>.fgb or .tif), params.json, script.R, and bootstrap.R, and spawns Rscript --vanilla bootstrap.R with Stdio::piped() attached to stdout and stderr4. The --vanilla flag prevents user .Rprofile scripts or .RData workspaces from polluting the execution environment. Bounded reader threads stream stdout and stderr lines into the job's progress notices, enforce timeout_ms via a watchdog timer that calls Child::kill() if exceeded, deserialize out/manifest.json and the generated output layer files into an RJobResponse, remove the temporary directory, and invoke gp.finish2.

## Data Exchange Wire Protocol and Memory Profiling

### Serialization Bottlenecks and the WebAssembly Memory Ceiling

Designing the data exchange contract between rustgis-data (L1) and the R runtime requires addressing two critical memory and binary-size constraints documented in the RustGIS repository and R package ecosystem4. First, Issue #140 (crates/engine/src/cmd/raster.rs) documents why the web application enforces a strict 256 MB upload cap (MAX_UPLOAD_BYTES): reading a 436 MB file (such as an EnMAP L2A spectral image with 224 bands stored as a ZSTD Cloud-Optimized GeoTIFF) into memory, base64-encoding it for layer.addDataBytes, decoding it again, and retaining the bytes in RasterLayer::from_bytes requires over 1.5 GB ([figure omitted], a [figure omitted] memory amplification factor) of 32-bit WebAssembly linear memory at its peak6. Passing byte arrays to the engine or across worker boundaries without a base64 round-trip keeps memory usage proportional to the raw payload size6. Consequently, rustgis-r strictly prohibits base64-encoding vector or raster layers inside JSON strings when crossing Services::r_run or WebR worker boundaries6.  
Second, the root Cargo.toml includes parquet = "60", arrow-array = "60", and arrow-schema = "60" (all with default-features = false), accompanied by an explicit architectural note that vector data providers read GeoParquet without pulling in the full Arrow runtime, which would make the web app's WebAssembly binary far larger (Arrow is used only by tests to write sample files)4. A parallel trade-off exists in the R ecosystem, as reflected in R-universe and CRAN metrics across 29,000 R packages and the 113-package (6.79 GB) r-multiverse collection5:

| Ecosystem Component / Metric | Size / Dependency Footprint | R-Universe / Repository Metrics | Architectural Implication for rustgis-r |
| :---- | :---- | :---- | :---- |
| **RustGIS Wasm Upload Cap (MAX_UPLOAD_BYTES)** | **256 MB** ceiling (crates/engine/src/cmd/raster.rs)6. | A **436 MB** 224-band ZSTD COG spikes to **>1,500 MB (1.5 GB)** peak Wasm memory under base64 layer.addDataBytes round-tripping6. | Eliminate base64 JSON encoding; pass raw Vec<u8> / Uint8Array buffers directly into MEMFS or OS files6. |
| **sf (Simple Features for R)** | Core spatial vector package (GEOS / GDAL / PROJ)5. | Score **22.69**, **1,453** stars, **1,512** dependents, **338,759** downloads20. | Primary R spatial vector representation; natively reads and writes FlatGeobuf and GeoJSON4. |
| **nanoarrow + geoarrow + wk** | Minimal C/R columnar and WKB interchange stack5. | nanoarrow: Score **12.92**, **250** stars, **59** dependents, **32,766** downloads; explicitly integrated with sf, wk, and geoarrow for WebR VFS images5. | Preferred zero-copy columnar/tabular bridge inside WebR without C++ Arrow binary bloat4. |
| **arrow (Full Apache Arrow C++ for R)** | Heavy C++ library (arrow, parquet, libxml2, curl, openssl)5. | Score **19.95**, **17k** stars, **219** dependents, **27k** scripts, **187k** downloads5. | Supported on Desktop Rscript when installed, but avoided as a required dependency in WebR4. |
| **Core R Infrastructure (rlang, vctrs, cli, igraph)** | Foundation condition, vector, console, and network libraries5. | rlang (**18k** dependents, **2.6M** downloads, score **21.02**); cli (**17k** dependents, **2.1M** downloads, score **19.45**); vctrs (**16k** dependents, **2.1M** downloads, score **20.39**); igraph (**2.2k** dependents, **521k** downloads, score **21.26**)5. | Pre-cached via WebR IDBFS so spatial and network analysis tools execute without repeated network downloads5. |

### Comparative Analysis of Geospatial Wire Formats

To balance zero-copy binary performance, Wasm bundle size constraints in Cargo.toml, and compatibility with existing ToolOutput schemas in rustgis-plugins, rustgis-r implements a tiered binary and JSON wire protocol4:

| Wire Format | RustGIS Dependencies (Cargo.toml) | R / WebR Reader Stack | Serialization & Memory Overhead | Designated Role in rustgis-r |
| :---- | :---- | :---- | :---- | :---- |
| **FlatGeobuf (RWireFormat::FlatGeobuf)** | flatgeobuf = "6", geozero = "0.15" (with-wkb), bytes = "1" (already compiled in rustgis-data for both native and Wasm)4. | sf::st_read("in/input.fgb", quiet = TRUE) or wk WKB reader5. | **Low (~1.05x raw size):** Binary IEEE-754 WKB coordinates + packed attributes; zero text or base64 overhead4. | **Primary Vector Format** for all layers exceeding 1,000 features or 1 MB across both Desktop and WebR4. |
| **GeoJSON (RWireFormat::GeoJson)** | serde_json = "1" (core workspace foundation dependency)4. | sf::st_read("in/input.geojson") or jsonlite::fromJSON()8. | **High (3.0x–5.0x raw size):** UTF-8 decimal coordinate formatting and JSON DOM parsing overhead6. | **Small-Layer & Plugin Parity Format** for layers < 1,000 features and direct compatibility with rustgis-plugins ToolOutput8. |
| **GeoTIFF (RWireFormat::GeoTiff)** | tiff = "0.11", proj4rs = "0.2", crs-definitions = "0.6" (pure Rust in rustgis-data)2. | terra::rast("in/raster.tif") or stars::read_stars() on Desktop; tiff / stars on WebR4. | **Minimal (~1.0x):** Raw RasterLayer bytes passed directly as binary Uint8Array / file without base64 encoding6. | **Primary Raster Format** for all single-band and multi-band GeoTIFF/COG layers1. |
| **Arrow IPC / GeoArrow (RWireFormat::ArrowIpc)** | arrow-array = "60", arrow-schema = "60" (Desktop/CLI only; excluded from Wasm runtime to prevent bundle bloat)4. | nanoarrow::read_nanoarrow() + geoarrow + wk (zero-copy columnar ingestion in R and WebR)5. | **Ultra-Low (~1.0x, zero-copy):** Columnar buffers aligned for analytical table outputs5. | **Tabular & Columnar Format** for standalone statistical tables and nanoarrow streams5. |

### Exact IPC Data Contract and Bootstrap Harness

To prevent any R script from corrupting the IPC channel when calling print(), cat(), message(), or warning(), standard output and standard error are strictly reserved for human-readable progress notices and diagnostic logs, never for returning geometry payloads7. All structured outputs are written to the job's isolated out/ directory (MEMFS in WebR, temporary directory on Desktop) alongside an out/manifest.json file that mirrors the ToolOutput schema established by rustgis-plugins (layers, tables, selection, messages)8.

#### Input params.json Schema

The host runner writes the job parameters and input layer descriptors to <job_dir>/params.json prior to invoking R2:

JSON  
{  
  "job_id": 42,  
  "tool_id": "r:local-moran",  
  "lang": "en",  
  "crs_epsg": 4326,  
  "inputs": {  
    "polygons": {  
      "layer_name": "Countries",  
      "path": "in/polygons.fgb",  
      "format": "FlatGeobuf",  
      "feature_count": 177  
    }  
  },  
  "params": {  
    "field": "gdp_md",  
    "permutations": 999,  
    "output_name": "Countries_LISA"  
  }  
}

#### Output out/manifest.json Schema

Upon completion, the R bootstrap harness writes <job_dir>/out/manifest.json, which the host runner parses to construct the RJobResponse for gp.finish2:

JSON  
{  
  "layers": [  
    {  
      "name": "Countries_LISA",  
      "file": "Countries_LISA.fgb",  
      "format": "FlatGeobuf"  
    }  
  ],  
  "tables": [  
    {  
      "name": "Moran_Global_Summary",  
      "columns": [  
        {"name": "statistic", "type": "string"},  
        {"name": "value", "type": "float64"},  
        {"name": "p_value", "type": "float64"}  
      ],  
      "rows": [  
        ["Global Moran I", 0.4182, 0.001]  
      ]  
    }  
  ],  
  "selection": {  
    "layer": "Countries",  
    "rows": [12, 45, 88, 104]  
  },  
  "messages": [  
    "Computed Local Moran's I across 177 features (999 conditional permutations)."  
  ]  
}

#### Standard R Bootstrap Harness

The crates/r-runner crate injects a locked bootstrap.R preamble into <job_dir>/bootstrap.R that provides helper functions (rgis_param, rgis_read_layer, rgis_write_layer, rgis_write_table, rgis_select, and rgis_log) to the tool or user script while isolating execution inside a child environment8:

R  

```
# <job_dir>/bootstrap.R — Injected by crates/r-runner  
.rgis_env <- new.env(parent = emptyenv())  
.rgis_env$manifest <- list(layers = list(), tables = list(), selection = NULL, messages = character(0))  
.rgis_env$params_doc <- jsonlite::fromJSON("params.json", simplifyVector = FALSE)
```

rgis_param <- function(name, default = NULL) {  
  val <- .rgis_env$params_doc$params[[name]]  
  if (is.null(val)) default else val  
}

rgis_read_layer <- function(param_name) {  
  spec <- .rgis_env$params_doc$inputs[[param_name]]  
  if (is.null(spec)) stop(sprintf("Missing input layer parameter: %s", param_name), call. = FALSE)  
  sf::st_read(spec$path, quiet = TRUE)  
}

rgis_write_layer <- function(sf_obj, name, format = "FlatGeobuf") {  
  safe_name <- gsub("[^A-Za-z0-9_\\-]", "_", name)  
  ext <- if (identical(format, "FlatGeobuf")) "fgb" else "geojson"  
  driver <- if (identical(format, "FlatGeobuf")) "FlatGeobuf" else "GeoJSON"  
  out_file <- sprintf("%s.%s", safe_name, ext)  
  sf::st_write(sf_obj, file.path("out", out_file), driver = driver, quiet = TRUE, delete_dsn = TRUE)  
  .rgis_env$manifest$layers[[length(.rgis_env$manifest$layers) + 1L]] <- list(  
    name = name, file = out_file, format = format  
  )  
}

rgis_log <- function(msg) {  
  .rgis_env$manifest$messages <- c(.rgis_env$manifest$messages, as.character(msg))  
  cat(sprintf("[RustGIS-R] %s\n", msg), file = stdout())  
}

```
# Execute the target tool or user script in an isolated child environment  
source("script.R", local = new.env(parent = globalenv()))
```

```
# Atomically flush manifest.json for host runner collection  
jsonlite::write_json(  
  .rgis_env$manifest,  
  path = file.path("out", "manifest.json"),  
  auto_unbox = TRUE,  
  null = "null",  
  digits = NA  
)
```

## Risk, Dependency, and Verification Matrix

Integrating an external statistical runtime across native desktop installers, headless CLI pipelines, Python wheels, and browser WebAssembly requires explicit mitigation of build, security, memory, and localization risks1.

| Risk Category | Architectural Vulnerability / Barrier | Impact Severity | Mitigation Strategy & Verification Gate |
| :---- | :---- | :---- | :---- |
| **Wasm Memory Exhaustion (rustgis-web)** | Large vector or raster layers transferred to WebR via JSON/base64 exceed 32-bit Wasm 4 GB address space (as seen in Issue #140 where a 436 MB COG reaches >1.5 GB)6. | **Critical** | Enforce raw Uint8Array binary transfers (FlatGeobuf and GeoTIFF) directly into Emscripten MEMFS via webR.FS.writeFile; enforce MAX_UPLOAD_BYTES (256 MB) per artifact and unlink() job directories in finally blocks6. |
| **COOP/COEP Header Conflict (rustgis-tiles)** | Enabling Cross-Origin-Embedder-Policy: require-corp for WebR SharedArrayBuffer blocks external WMS/XYZ basemap tiles and breaks rustgis.Map Jupyter iframes3. | **High** | Default to ChannelType.PostMessage whenever window.crossOriginIsolated === false16. Use non-interactive shelter.captureR() batch execution, which requires no synchronous SharedArrayBuffer stdin blocking7. |
| **Hostile Parameter & Script Panics** | Malformed JSON parameters, missing fields, or NaN coordinates passed to gp.run or r.runScript could trigger panics or unbounded memory allocation2. | **High** | Enforce #![forbid(unsafe_code)] and #![deny(clippy::unwrap_used, ...)] in crates/r-runner; register all r:* tools and r.runScript in crates/engine/src/tests.rs::hostile_params_never_panic2. |
| **R Infinite Loops / Native C Segfaults** | User R script enters an infinite loop or triggers a C segfault inside sf/GEOS on self-intersecting polygons. | **High** | On Desktop/CLI, isolate execution inside a child Rscript --vanilla process with a configurable watchdog timer (timeout_ms, default 120s) calling Child::kill(); on WebR, call webR.close() and respawn the Web Worker on timeout2. |
| **Unauthorized Execution via MCP / AI / Plugins** | Malicious prompt injection via rustgis-cli mcp, rustgis-assistant, or a third-party .rgplugin executes arbitrary R system commands (system())2. | **Critical** | Hard-block Services::r_run inside rustgis-cli mcp (matching geolibre and scripts policy in docs/mcp.md); require explicit user approval in rustgis-assistant; forbid r.* and r:* tool calls from rustgis-plugins2. |
| **Missing Native R Installation on Desktop** | Desktop users installing rustgis via winget, brew, or AppImage may not have R or sf installed locally1. | **Medium** | Treat Rscript identically to the optional geolibre native binary: probe $RUSTGIS_RSCRIPT_PATH and PATH gracefully, and emit a localized notice via tl!("...") with one-command package installation instructions if missing2. |
| **Internationalization & Catalog Parity Drift** | New commands (r.runScript) or UI labels fail CI parity floor checks or miss translations across the 16 interface languages1. | **Medium** | Register r.runScript in crates/engine/src/catalog.rs (verified by rustgis-cli parity), wrap all UI literals in tl!("...") / i18n::fmt(...), and populate all 16 crates/ui-egui/src/i18n/*.tsv files1. |

## Architectural Conclusions and Implementation Roadmap

The design of rustgis-r (crates/r-runner) demonstrates that a full-featured statistical R runtime can be integrated into RustGIS without compromising any of the workspace's foundational invariants2. By situating rustgis-r at Layer 3 (L3) parallel to rustgis-geolibre and routing execution through the split-phase external_start → Services::r_run → gp.finish pipeline, the engine maintains a single, unified transaction and undo model across native desktop binaries, CLI scripts, Python notebooks, and WebAssembly in the browser1. Adopting out-of-process Rscript --vanilla execution on native platforms preserves #![forbid(unsafe_code)] and guarantees that C or Fortran faults inside R packages can never crash the RustGIS host, while pairing WebR's ChannelType.PostMessage worker mode with binary FlatGeobuf and GeoTIFF virtual filesystem staging eliminates both Cross-Origin Isolation basemap regressions and the [figure omitted] base64 WebAssembly memory amplification bottleneck2.  
Implementation proceeds across four sequential phases verified by cargo xtask ci1. In the first phase, crates/r-runner is scaffolded at Layer 3 with #![forbid(unsafe_code)] and strict Clippy denials, defining RJobRequest, RJobResponse, RWireFormat, and FlatGeobuf, GeoJSON, and GeoTIFF staging serializers atop rustgis-data (L1) and rustgis-analysis (L2), while verifying that cargo xtask layers and cargo build --target wasm32-unknown-unknown -p rustgis-web pass cleanly2. In the second phase, crates/engine (L4) adds the r-runtime feature flag, extends Services with fn r_run(&mut self, job: RJobRequest), registers r.runScript and r:* tool dispatch in crates/engine/src/cmd/gp.rs, wires gp.finish to commit returned ROutputLayer buffers and RSelectionOutput rows as a single undo step, enforces MCP, AI Assistant approval, and plugin sandboxing blocks, and adds fuzz tests to crates/engine/src/tests.rs::hostile_params_never_panic2. In the third phase, crates/r-runner/src/native.rs implements the non-Wasm Rscript --vanilla subprocess runner with temporary directory staging, bootstrap.R injection, bounded stdout and stderr pipe readers, and watchdog timeout termination across apps/rustgis, apps/rustgis-cli, and apps/rustgis-python1. In the final phase, apps/rustgis-web and crates/ui-egui (L6) integrate webr_bridge.js with lazy WebR Dedicated Worker initialization, ChannelType.PostMessage fallback, Emscripten MEMFS staging, IDBFS package caching for sf, wk, and nanoarrow, and localized R Script Editor UI panes verified by headless ui_shot screenshots and rustgis-cli parity2.

#### Works cited

> 1. opengeos/RustGIS: A fast, free and open-source GIS in pure Rust., [https://github.com/opengeos/RustGIS](https://github.com/opengeos/RustGIS)  
> 2. RustGIS/AGENTS.md at main - GitHub, [https://github.com/opengeos/RustGIS/blob/main/AGENTS.md](https://github.com/opengeos/RustGIS/blob/main/AGENTS.md)  
> 3. RustGIS/docs/python.md at main - GitHub, [https://github.com/opengeos/RustGIS/blob/main/docs/python.md](https://github.com/opengeos/RustGIS/blob/main/docs/python.md)  
> 4. Cargo.toml - opengeos/RustGIS - GitHub, [https://github.com/opengeos/RustGIS/blob/main/Cargo.toml](https://github.com/opengeos/RustGIS/blob/main/Cargo.toml)  
> 5. R-universe - r-multiverse (R-multiverse), [https://community.r-multiverse.org/](https://community.r-multiverse.org/)  
> 6. open large local COGs by byte range instead of reading the whole, [https://github.com/opengeos/RustGIS/issues/140](https://github.com/opengeos/RustGIS/issues/140)  
> 7. Embedding R in WordPress using webR - Hypebright, [https://hypebright.nl/en/r-en/embedding-r-in-wordpress-using-webr-2/](https://hypebright.nl/en/r-en/embedding-r-in-wordpress-using-webr-2/)  
> 8. RustGIS/docs/plugins.md at main - GitHub, [https://github.com/opengeos/RustGIS/blob/main/docs/plugins.md](https://github.com/opengeos/RustGIS/blob/main/docs/plugins.md)  
> 9. RustGIS/ROADMAP.md at main · opengeos/RustGIS - GitHub, [https://github.com/opengeos/RustGIS/blob/main/ROADMAP.md](https://github.com/opengeos/RustGIS/blob/main/ROADMAP.md)  
> 10. RustGIS/docs/cli.md at main - opengeos - GitHub, [https://github.com/opengeos/RustGIS/blob/main/docs/cli.md](https://github.com/opengeos/RustGIS/blob/main/docs/cli.md)  
> 11. RustGIS/docs/mcp.md at main - GitHub, [https://github.com/opengeos/RustGIS/blob/main/docs/mcp.md](https://github.com/opengeos/RustGIS/blob/main/docs/mcp.md)  
> 12. RustGIS/docs/installation.md at main - GitHub, [https://github.com/opengeos/RustGIS/blob/main/docs/installation.md](https://github.com/opengeos/RustGIS/blob/main/docs/installation.md)  
> 13. R in the Browser - Evaluating R Code - WebR, [https://docs.r-wasm.org/webr/v0.1.0/evaluating.html](https://docs.r-wasm.org/webr/v0.1.0/evaluating.html)  
> 14. R in the Browser - Examples using WebR, [https://docs.r-wasm.org/webr/v0.2.0/examples.html](https://docs.r-wasm.org/webr/v0.2.0/examples.html)  
> 15. Evaluating R Code – WebR - R in the Browser, [https://docs.r-wasm.org/webr/latest/evaluating.html](https://docs.r-wasm.org/webr/latest/evaluating.html)  
> 16. Serving Pages with WebR – WebR - R in the Browser, [https://docs.r-wasm.org/webr/v0.5.4/serving.html](https://docs.r-wasm.org/webr/v0.5.4/serving.html)  
> 17. Airsequel/awesome-sqlite - GitHub, [https://github.com/Airsequel/awesome-sqlite](https://github.com/Airsequel/awesome-sqlite)  
> 18. CRAN: Available Packages by Date - R Project, [ftp://www.r-project.org/pub/R/web/packages/available_packages_by_date.html](ftp://www.r-project.org/pub/R/web/packages/available_packages_by_date.html)  
> 19. Is there a way to run R code from JavaScript? - Stack Overflow, [https://stackoverflow.com/questions/17665565/is-there-a-way-to-run-r-code-from-javascript](https://stackoverflow.com/questions/17665565/is-there-a-way-to-run-r-code-from-javascript)  
> 20. top packages - R-universe, [https://r-universe.dev/packages](https://r-universe.dev/packages)  
> 21. R Package Stats, Search and Tutorials | How is this Package doing?, [https://rpkg.net/](https://rpkg.net/)








## Erratum

These corrections apply to the sections above. The RFC body is kept as written.

1. **Flags.** The RFC says `Rscript --vanilla` (lines 297, 302, 459, 466, 467). The code runs `Rscript --no-save --no-restore --no-init-file bootstrap.R` (`crates/r/src/native.rs:88`). `--vanilla` would also skip `.Renviron`, which RustGIS keeps. See MASTER_PLAN D9.
2. **Catalog.** `crates/engine/src/catalog.rs` does not exist. Commands register in `crates/engine/src/cmd/gp.rs`, not in a catalog file (`r.runScript` in line 462 is `gp.runR` in code).
3. **Crate path.** The R runner is `crates/r` (package `rustgis-r`), not `crates/r-runner`.
4. **Scope.** The runner is native only: desktop, CLI and Python. The webR bridge is future work behind the `Services::r_run` hook. The web app sets that hook to `None`, so `gp.runR` fails cleanly in the browser.
