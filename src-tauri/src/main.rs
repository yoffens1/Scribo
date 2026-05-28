//! # Scribo Main Entrypoint
//!
//! Evaluates CLI parameters and decides whether to launch the CLI handler
//! or startup the GUI window via Tauri.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

/// The absolute entrypoint of the Scribo binary.
/// If CLI arguments are provided, routes execution to the CLI driver.
/// Otherwise, starts the desktop application.
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        scribo_lib::cli::handle_cli(args);
    } else {
        scribo_lib::run();
    }
}
