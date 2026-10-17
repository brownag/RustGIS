# R Scripts

RustGIS can run an R script on the layers in your project. The desktop app, the command line
and the Python package run it with a local R installation. The browser app does not run R yet.

A script reads the project's layers and writes new tables and layers back. A run is one undo
step (++ctrl+z++); it never changes the layers it reads.

## Install R

Install R from [CRAN](https://cran.r-project.org/), then start R and run:

```r
install.packages(c("jsonlite", "sf"))
```

`jsonlite` reads and writes the run's files; `sf` is needed for layers with geometry. RustGIS
never installs packages: a run that lacks one stops and names the `install.packages` call.

## Locate Rscript

RustGIS runs the `Rscript` program that comes with R. On the desktop, run **Locate Rscript…**
from command search (under **Analyze › Geoprocessing**) and pick the program. From the command
line or Python, use **Set Rscript** (`gp.setRscript`) with the path. RustGIS refuses a program
that is not Rscript.

RustGIS uses the first Rscript it finds: your choice, `RUSTGIS_RSCRIPT`, `R_HOME`, `PATH`, then
the usual install folders.

## Running it

`gp.runR` takes these parameters:

| Parameter | What it is |
| --- | --- |
| `script` | The R code. Required. At most 1 MB. |
| `inputs` | The names of the layers the script reads, as a list. Optional. |
| `args` | An object of values the script reads with `rgis_param` and `rgis_args`. Optional. |
| `timeout_s` | The time limit in whole seconds. Optional; see [Limits](#limits). |
| `lang` | The interface language for R's own messages. Optional. The desktop app sets it. |

Example: count the **Cities** layer, with a 60-second limit:

```sh
rustgis-cli run --sample --cmd 'gp.runR={"script":"cities <- rgis_read_layer(\"Cities\")\nrgis_write_table(data.frame(cities = nrow(cities)), \"City count\")","inputs":["Cities"],"timeout_s":60}'
```

The command line and Python wait for the run and use English R messages.

## What a script reads and writes

RustGIS loads these functions before your code runs.

| Function | What it does |
| --- | --- |
| `rgis_param(name, default)` | Reads one value from `args`, or `default` when it is missing. |
| `rgis_args()` | Returns all of `args` as a named list. |
| `rgis_log(..., level)` | Records a message for the run's log (`debug`, `info`, `warn`, `error`). |
| `rgis_read_layer(name)` | Reads a layer named in `inputs`; only selected rows if it has a selection. |
| `rgis_select(layer, rows)` | Sets the rows the run returns for a layer (1-based). The last call wins. |
| `rgis_write_layer(x, name)` | Adds an `sf` object as a new layer. Needs `sf`. |
| `rgis_write_table(df, name)` | Adds a data frame as a new table. |

Layer and output names are checked. Printed text (`print`, `cat`, `message`) goes to the run's messages.

## Limits

The script is at most 1 MB. The time limit defaults to 120 seconds (1 to 86,400 allowed). One R
run goes at a time; a second run waits in a queue. A run that fails or times out is stopped: the
project is unchanged, the reason shows as a notice, and the run is not added to the history.

## Who may run R

R is reserved to you.

- **You** can run R from the command line, Python, or the desktop app's commands.
- **Plugins** and **MCP clients** cannot run R ([MCP](../mcp.md)).
- **The AI Assistant** may ask, but asks you every time ([AI Assistant](assistant.md)).

A script can do anything your user account can do, so only run scripts you trust.
