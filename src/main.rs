pub mod editor;
pub mod renderer;
pub mod app;
pub mod terminal;
pub mod git;
pub mod experiments;
pub mod machkit;

fn main() {
    experiments::startup::record_start_time();

    // Direct driver/hardware optimizations for Intel/Mesa/NVIDIA
    unsafe {
        std::env::set_var("MESA_NO_ERROR", "1");                 // Skip driver-level error checks for performance
        std::env::set_var("mesa_glthread", "true");              // Threaded GL pipeline execution
        std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1");   // Threaded NVIDIA execution
    }

    let mut args = std::env::args().skip(1);
    let mut file_path = None;
    let mut experimental = false;

    while let Some(arg) = args.next() {
        if arg == "--experimental" || arg == "-experimental" {
            experimental = true;
        } else if file_path.is_none() {
            file_path = Some(arg);
        }
    }

    if let Err(e) = app::run_editor(file_path, experimental) {
        eprintln!("Error running editor: {}", e);
        std::process::exit(1);
    }
}