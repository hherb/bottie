//! Native Bottie executable entry point.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Starts the shared Bottie application library.
fn main() {
    bottie_lib::run()
}
