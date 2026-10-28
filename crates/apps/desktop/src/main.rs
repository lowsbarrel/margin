#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod container;
mod history_snapshots;
mod network;
mod vault_reader;
mod window;

fn main() {
    app::run();
}
