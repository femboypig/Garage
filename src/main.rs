#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::manual_memcpy,
    clippy::unnecessary_unwrap,
    clippy::if_same_then_else,
    clippy::new_without_default,
    clippy::manual_strip,
    clippy::manual_clamp,
    clippy::bool_assert_comparison,
    clippy::manual_flatten
)]

pub mod app;
pub mod editor;
pub mod experiments;
pub mod git;
pub mod machkit;
pub mod renderer;
pub mod terminal;

fn main() {
    experiments::startup::record_start_time();

    // Direct driver/hardware optimizations for Intel/Mesa/NVIDIA
    unsafe {
        std::env::set_var("MESA_NO_ERROR", "1"); // Skip driver-level error checks for performance
        std::env::set_var("mesa_glthread", "true"); // Threaded GL pipeline execution
        std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1"); // Threaded NVIDIA execution
    }

    let args = std::env::args().skip(1);
    let mut file_path = None;
    let mut experimental = false;

    for arg in args {
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
