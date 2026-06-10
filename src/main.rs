pub mod editor;
pub mod renderer;
pub mod ui;
pub mod window;
pub mod terminal;

fn main() {
    let file_path = std::env::args().nth(1);
    if let Err(e) = window::run_editor(file_path) {
        eprintln!("Error running editor: {}", e);
        std::process::exit(1);
    }
}
