# RustGIS R harness. Runs one job from its job directory (the working directory):
#   Rscript --no-save --no-restore --no-init-file bootstrap.R
#
# Inputs, written by the host:
#   params.json      {"version":1, "lang", "timeout_s", "args", "inputs":[...]}; each input has
#                    index, name, kind (vector|table|raster), format, file, and selection
#                    (1-based rows, empty when nothing is selected)
#   in/<index>.<ext> the input layers (fgb, csv, tif)
#   script.R         the user script
#
# Outputs: files the script writes go to out/, and out/manifest.json lists them:
#   {"version":1, "ok":true, "error":null, "layers":[{"name","file","kind","format"}],
#    "selection":{"layer","rows"} or null, "messages":[{"level","text"}]}
# A script error writes ok:false and the error text. A script may return layers or a
# selection, not both. The host treats a missing or invalid manifest as a failure. If
# params.json or out/ cannot be read or made, the harness stops with no manifest. Otherwise
# it exits 0 once the manifest is written.
#
# Helpers live in globalenv(), so the script (run in a child of globalenv()) can call them.
# Private state lives in .rgis_state.

.rgis_state <- new.env(parent = emptyenv())
.rgis_state$messages <- list()
.rgis_state$layers <- list()
.rgis_state$selection <- list()
.rgis_state$args <- list()
.rgis_state$inputs <- list()

# Stops with a plain install message when a package is missing.
.rgis_need <- function(pkg, msg) {
  if (!requireNamespace(pkg, quietly = TRUE)) stop(msg, call. = FALSE)
  invisible(TRUE)
}

# A layer name must be one non-empty string.
.rgis_check_name <- function(name) {
  if (!is.character(name) || length(name) != 1L || is.na(name) || !nzchar(name)) {
    stop("a layer name must be one non-empty string", call. = FALSE)
  }
  invisible(name)
}

# Finds an input by name: exact match first, then case-insensitive.
.rgis_find_input <- function(name) {
  inputs <- .rgis_state$inputs
  for (inp in inputs) {
    if (identical(inp[["name"]], name)) return(inp)
  }
  for (inp in inputs) {
    if (identical(tolower(inp[["name"]]), tolower(name))) return(inp)
  }
  available <- vapply(inputs, function(inp) as.character(inp[["name"]]), "")
  stop(sprintf("no input layer named '%s'; available: %s", name,
               if (length(available)) paste(available, collapse = ", ") else "none"),
       call. = FALSE)
}

# Adds a written layer to the manifest list. The file name is the index, never the name.
.rgis_add_layer <- function(name, file, kind, format) {
  n <- length(.rgis_state$layers) + 1L
  .rgis_state$layers[[n]] <- list(name = name, file = file, kind = kind, format = format)
  invisible(file)
}

# The layer name and rows for each layer that has a selection.
.rgis_chosen <- function() {
  sel <- .rgis_state$selection
  sel[lengths(sel) > 0L]
}

# Builds the manifest list. A script error, or a shape the host cannot convert, gives ok:false.
.rgis_manifest <- function(error) {
  layers <- .rgis_state$layers
  sel <- .rgis_chosen()
  if (is.null(error)) {
    if (length(layers) > 0L && length(sel) > 0L) {
      error <- "a script can return layers or a selection, not both"
    } else if (length(sel) > 1L) {
      error <- "a script can select rows on one layer only"
    }
  }
  if (!is.null(error)) {
    return(list(version = 1L, ok = FALSE, error = error, layers = list(), selection = NULL,
                messages = .rgis_state$messages))
  }
  selection <- NULL
  if (length(sel) == 1L) {
    selection <- list(layer = names(sel)[[1L]], rows = I(sel[[1L]]))
  }
  list(version = 1L, ok = TRUE, error = NULL, layers = layers, selection = selection,
       messages = .rgis_state$messages)
}

# Runs the user script in a child environment and writes out/manifest.json.
.rgis_run <- function() {
  error <- tryCatch({
    withCallingHandlers(
      source("script.R", local = new.env(parent = globalenv())),
      warning = function(w) {
        rgis_log(conditionMessage(w), level = "warn")
        invokeRestart("muffleWarning")
      })
    NULL
  }, error = function(e) {
    msg <- conditionMessage(e)
    if (nzchar(msg)) msg else "the script failed"
  })
  manifest <- .rgis_manifest(error)
  jsonlite::write_json(manifest, file.path("out", "manifest.json"), auto_unbox = TRUE,
                       null = "null", na = "null")
  invisible(manifest)
}

# Returns the value of an argument, or default when the argument is not set.
rgis_param <- function(name, default = NULL) {
  val <- .rgis_state$args[[name]]
  if (is.null(val)) default else val
}

# Returns all arguments as a named list.
rgis_args <- function() {
  .rgis_state$args
}

# Records a log line in the manifest messages. Nothing is printed.
rgis_log <- function(..., level = "info") {
  level <- as.character(level)[1L]
  if (!level %in% c("debug", "info", "warn", "error")) {
    stop("log level must be one of debug, info, warn, error", call. = FALSE)
  }
  text <- paste0(..., collapse = "")
  n <- length(.rgis_state$messages) + 1L
  .rgis_state$messages[[n]] <- list(level = level, text = text)
  invisible(text)
}

# Stores the 1-based rows selected on a layer. The last call for a layer wins.
rgis_select <- function(layer, rows) {
  .rgis_check_name(layer)
  if (!is.numeric(rows) || any(!is.finite(rows)) || any(rows < 1) ||
      any(rows != floor(rows)) || any(rows > .Machine$integer.max)) {
    stop("selected rows must be whole numbers, 1 or more", call. = FALSE)
  }
  .rgis_state$selection[[layer]] <- sort(unique(as.integer(rows)))
  invisible(rows)
}

# Reads an input layer. A vector is restricted to its selection when one is set.
rgis_read_layer <- function(name) {
  inp <- .rgis_find_input(name)
  file <- inp[["file"]]
  kind <- inp[["kind"]]
  if (identical(kind, "vector")) {
    .rgis_need("sf", "this script needs geometry; install the R package 'sf': install.packages(\"sf\")")
    x <- sf::st_read(file, quiet = TRUE)
    sel <- unlist(inp[["selection"]])
    if (length(sel)) x <- x[sel[sel <= nrow(x)], ]
    return(x)
  }
  if (identical(kind, "table")) {
    return(utils::read.csv(file, stringsAsFactors = FALSE))
  }
  if (identical(kind, "raster")) {
    .rgis_need("terra", "this script needs rasters; install the R package 'terra': install.packages(\"terra\")")
    return(terra::rast(file))
  }
  stop(sprintf("input layer '%s' has an unknown kind", name), call. = FALSE)
}

# Writes an sf layer to out/ as EPSG:4326. FlatGeobuf is used when available, else GeoJSON.
rgis_write_layer <- function(x, name) {
  .rgis_check_name(name)
  .rgis_need("sf", "this script needs geometry; install the R package 'sf': install.packages(\"sf\")")
  if (!inherits(x, "sf")) {
    stop("rgis_write_layer needs an sf object; use rgis_write_table for a table", call. = FALSE)
  }
  if (is.na(sf::st_crs(x))) {
    bb <- sf::st_bbox(x)
    out_of_range <- all(is.finite(bb)) &&
      (bb[["xmin"]] < -180 || bb[["xmax"]] > 180 || bb[["ymin"]] < -90 || bb[["ymax"]] > 90)
    if (out_of_range) {
      stop("the layer looks projected but has no CRS; set one with sf::st_set_crs()", call. = FALSE)
    }
    x <- sf::st_set_crs(x, 4326)
  } else {
    x <- sf::st_transform(x, 4326)
  }
  drv <- sf::st_drivers("vector")
  has_fgb <- any(drv$name == "FlatGeobuf" & drv$write)
  n <- length(.rgis_state$layers)
  if (has_fgb) {
    file <- sprintf("out/%d.fgb", n)
    sf::st_write(x, file, driver = "FlatGeobuf", quiet = TRUE, delete_dsn = file.exists(file))
    format <- "fgb"
  } else {
    file <- sprintf("out/%d.geojson", n)
    sf::st_write(x, file, driver = "GeoJSON", quiet = TRUE, delete_dsn = file.exists(file))
    format <- "geojson"
  }
  .rgis_add_layer(name, file, "vector", format)
}

# Writes a data frame to out/ as CSV, with no row names.
rgis_write_table <- function(df, name) {
  .rgis_check_name(name)
  if (!is.data.frame(df)) stop("rgis_write_table needs a data frame", call. = FALSE)
  file <- sprintf("out/%d.csv", length(.rgis_state$layers))
  utils::write.csv(df, file, row.names = FALSE)
  .rgis_add_layer(name, file, "table", "csv")
}

# Reads params.json, sets up out/, then runs the script.
.rgis_main <- function() {
  .rgis_need("jsonlite", "RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")")
  params <- jsonlite::fromJSON("params.json", simplifyVector = FALSE)
  if (!is.list(params) || !isTRUE(params[["version"]] == 1)) {
    stop("params.json is not version 1", call. = FALSE)
  }
  lang <- params[["lang"]]
  if (!is.character(lang) || length(lang) != 1L || !nzchar(lang)) lang <- "en"
  Sys.setenv(LANGUAGE = lang)
  args <- params[["args"]]
  .rgis_state$args <- if (is.list(args)) args else list()
  inputs <- params[["inputs"]]
  .rgis_state$inputs <- if (is.list(inputs)) inputs else list()
  dir.create("out", showWarnings = FALSE, recursive = TRUE)
  if (!dir.exists("out")) stop("could not create the out/ directory", call. = FALSE)
  .rgis_run()
}

.rgis_main()
