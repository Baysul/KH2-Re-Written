//! The mod-facing action documentation must match the action table the
//! engine (and the setup's input.cfg check) uses.
use skate_core::input::gameplay_map::ACTIONS;

fn read(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
