//! The `gp.runR` runner in the browser. This is a placeholder: webR is the intended backend,
//! and it is not wired up yet, so every run reports that.

use rustgis_ui_egui::Slot;
use rustgis_ui_egui::rustgis_engine::cmd::gp::RRun;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function r_run() { throw new Error("webR is not wired up yet"); }
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn r_run() -> Result<JsValue, JsValue>;
}

fn js_error(e: JsValue) -> String {
    match e.dyn_ref::<js_sys::Error>() {
        Some(err) => String::from(err.message()),
        None => e.as_string().unwrap_or_else(|| format!("{e:?}")),
    }
}

/// Fills `slot` with the run's result. Until webR is wired up, that is always an error.
pub fn run(_job: RRun, slot: Slot<Result<serde_json::Value, String>>) {
    let r = r_run().map(|_| serde_json::Value::Null).map_err(js_error);
    if let Ok(mut s) = slot.lock() {
        *s = Some(r);
    }
}
