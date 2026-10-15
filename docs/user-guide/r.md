# R Scripts

RustGIS can run an R script on the layers in your project. The desktop app, the command line
and the Python package run the script with a local R installation (the native runner). The
browser app does not run R yet.

## What a script does

An R script reads the project's layers, works on them, and writes new tables and layers back.
Each run is one step: the new tables and layers are added to the project together, and one
undo (++ctrl+z++) takes them all away. A run never changes the layers it reads.

Run the **Run R Script** command (`gp.runR`) from the command line or Python, or ask the AI
Assistant to run it. The command takes the script text and the names of the layers it reads.
See [Running it](#running-it) below.

## Install R and the packages

Install R from [CRAN](https://cran.r-project.org/), then start R and install the two packages
that RustGIS's R support uses:

```r
install.packages(c("jsonlite", "sf"))
```

`jsonlite` reads and writes the run's files. `sf` reads and writes layers with geometry; a script
that only writes tables does not need it.

## Locate Rscript

RustGIS runs the `Rscript` program that comes with R. To pick a different one, use **Locate
Rscript** (`gp.setRscript`, under **Analyze › Geoprocessing**). Give it the path to the program;
RustGIS checks that it is an Rscript program and keeps it. Leave the path empty to look in the
usual places again.

The environment variable `RUSTGIS_RSCRIPT` names the program too. RustGIS reads it when it looks
for R, in the desktop app, the command line and Python alike.

RustGIS checks that the program runs and that `jsonlite` and `sf` load. If one of them is
missing, the message names the `install.packages` call to run.

## Where RustGIS looks for Rscript

RustGIS uses the first program that answers `--version` with an Rscript version line, looking in
this order:

1. The program you chose with **Locate Rscript**.
2. The path in `RUSTGIS_RSCRIPT`.
3. `Rscript` in the `bin` folder of the `R_HOME` folder.
4. `Rscript` in each folder on your `PATH`.
5. The usual install folders: on Windows, the newest `R-x.y.z` folder under `Program Files\R`;
   on Linux and macOS, `/usr/local/bin`, `/usr/bin`, `/opt/homebrew/bin` and the R framework
   folder on macOS.

## Running it

`gp.runR` takes these parameters:

| Parameter | What it is |
| --- | --- |
| `script` | The R code. Required. At most 1 MB. |
| `inputs` | The names of the layers the script reads, as a list. Optional. |
| `args` | An object of values the script reads with `rgis_param` and `rgis_args`. Optional. |
| `timeout_s` | The time limit in whole seconds. Optional; see [Limits](#limits). |

From the command line, this script reads the **Cities** layer, adds a one-row table with its
feature count, and gives it a 60-second limit:

```sh
rustgis-cli run --sample --cmd 'gp.runR={"script":"cities <- rgis_read_layer(\"Cities\")\nrgis_write_table(data.frame(cities = nrow(cities)), \"City count\")","inputs":["Cities"],"timeout_s":60}'
```

`gp.runR` returns after the result is applied to the project. The command line and Python
wait for the run to finish.

## The harness

The script calls these functions. RustGIS loads them before your code runs.

| Function | What it does |
| --- | --- |
| `rgis_param(name, default)` | Reads one value from `args`, or `default` when it is missing. |
| `rgis_args()` | Returns all of `args` as a named list. |
| `rgis_log(..., level)` | Records a message for the run's log. `level` is `debug`, `info`, `warn` or `error`. Nothing is printed. |
| `rgis_read_layer(name)` | Reads a layer named in `inputs`. A vector layer is limited to its selection, if one is set. |
| `rgis_select(layer, rows)` | Keeps only the given rows (1-based) of a layer for `rgis_read_layer`. The last call for a layer wins. |
| `rgis_write_layer(x, name)` | Adds an `sf` object as a new layer. Needs the `sf` package. |
| `rgis_write_table(df, name)` | Adds a data frame as a new table. |

Layer and output names are checked. A name that is not allowed stops the run with an error.

## Limits

- The script is at most 1 MB.
- A file that goes to or from a run is at most 512 MiB on the desktop, the command line and Python.
  A run takes or returns at most 256 files, and its manifest is at most 8 MB.
- A run keeps at most 1,000 log messages.
- The time limit defaults to 120 seconds. You can ask for any whole number of seconds from
  1 to 86,400 (24 hours). A negative value is treated as 1 second.

## When a run fails

A run that fails, or runs past its time limit, is stopped. Your layers and tables stay as they
were: nothing is added, and the project is not changed. The failed run is still listed in the
project's geoprocessing history, with the reason it failed.

Only one R run goes at a time. A second run waits in a queue, which has a fixed limit.

## Who may run R

- **You** can run R from the command line, Python, or the desktop app's commands.
- **Plugins** cannot. A plugin that asks to run `gp.runR`, or an R tool, is refused.
- **MCP clients** cannot. The MCP server refuses R runs, and the `rustgis-cli mcp --yes` flag does
  not allow them. See [MCP](../mcp.md).
- **The AI Assistant** may ask to run R, but it asks you every time. A run is never approved for
  the rest of the conversation. See [AI Assistant](assistant.md).

An R script can do anything your user account can do on this computer, so only run scripts you
trust.

## Platforms

The R CI job runs the R tests on Ubuntu, with R installed. The other CI jobs do not install R,
so the R tests skip there. Windows and macOS are untested for R runs. If a run fails on one of
them, please report it on the RustGIS issue tracker.
