use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};
use serde::Serialize;

/// `comfy_table::Table` preconfigured with the UTF-8 box preset and
/// dynamic column sizing — the only preset+arrangement combo any of
/// our commands ever uses.
pub fn new_table() -> Table {
    let mut t = Table::new();
    t.load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    t
}

/// Pretty-print `value` as JSON to stdout. On serialization failure, fall
/// back to logging to stderr so a `--json` flow at least exits cleanly
/// instead of producing partial output.
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("JSON output failed: {e}"),
    }
}
