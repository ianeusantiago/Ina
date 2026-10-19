// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Status line of Claude sessions started by INA (see usage.rs).
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == ina_lib::USAGE_SINK_FLAG {
        return ina_lib::usage_sink(std::path::Path::new(&args[2]));
    }
    ina_lib::run()
}
