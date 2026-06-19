use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static STEPS: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());

pub fn record_start_time() {
    let now = Instant::now();
    let _ = START_TIME.set(now);
    if let Ok(mut steps) = STEPS.lock() {
        steps.push(("Start", now));
    }
}

pub fn record_step(name: &'static str) {
    let now = Instant::now();
    if let Ok(mut steps) = STEPS.lock() {
        steps.push((name, now));
    }
}

pub fn report_startup_complete() {
    if let Some(start) = START_TIME.get() {
        let duration = start.elapsed();
        println!("Startup complete! Total Time elapsed: {:.2?}", duration);
        println!("Initialization timeline:");
        if let Ok(steps) = STEPS.lock() {
            for (name, time) in steps.iter() {
                if *name == "Start" {
                    continue;
                }
                let elapsed = time.duration_since(*start);
                println!("  - {}: {:.2?} absolute", name, elapsed);
            }
            let first_frame_elapsed = Instant::now().duration_since(*start);
            println!(
                "  - First Frame Rendered: {:.2?} absolute",
                first_frame_elapsed
            );
        }
    }
}
