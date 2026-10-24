// Windows release builds are GUI applications: without this attribute the
// binary opens a console window alongside the app. Debug builds keep the
// console so `println!`/panic output stays visible during development.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    markdown_blathy_lib::run()
}
