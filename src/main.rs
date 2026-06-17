pub mod editor;
pub mod renderer;
pub mod app;
pub mod terminal;
pub mod git;
pub mod experiments;
pub mod machkit;

fn main() {
    experiments::startup::record_start_time();
    let file_path = std::env::args().nth(1);
    if let Err(e) = app::run_editor(file_path) {
        eprintln!("Error running editor: {}", e);
        std::process::exit(1);
    }
}