// Hide the extra console window on Windows release builds (kept in dev for logs).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    health_tracker_lib::run();
}