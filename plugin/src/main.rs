use zelligent_plugin::State;
use zellij_tile::prelude::*;

register_plugin!(State);

// Stub for native builds: zellij-tile declares host_run_plugin_command as a WASM
// host import (#[link(wasm_import_module = "zellij")]). On native targets (used
// only for `cargo test`), the linker can't find it, so we provide a no-op stub.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn host_run_plugin_command() {}
