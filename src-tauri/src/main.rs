#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod headless;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        headless::handle_cli(args);
    } else {
        scribo_lib::run();
    }
}
