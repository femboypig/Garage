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
        std::env::set_var("vblank_mode", "0");                   // Disable vertical synchronization (Mesa)
        std::env::set_var("__GL_SYNC_TO_VBLANK", "0");           // Disable VSync (NVIDIA)
        std::env::set_var("MESA_NO_ERROR", "1");                 // Skip driver-level error checks for performance
        std::env::set_var("mesa_glthread", "true");              // Threaded GL pipeline execution
        std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1");   // Threaded NVIDIA execution
    }

    let file_path = std::env::args().nth(1);
    if let Err(e) = app::run_editor(file_path) {
        eprintln!("Error running editor: {}", e);
        std::process::exit(1);
    }
}