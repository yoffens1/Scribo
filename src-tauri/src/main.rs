#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        scribo_lib::cli::handle_cli(args);
    } else {
        scribo_lib::run();
    }
}
