use std::time::Instant;
use std::sync::{OnceLock, Mutex};

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
        println!("Initialization breakdown:");
        if let Ok(steps) = STEPS.lock() {
            let mut last_time = *start;
            for (name, time) in steps.iter() {
                if *name == "Start" {
                    continue;
                }
                let step_dur = time.duration_since(last_time);
                println!("  - {}: {:.2?}", name, step_dur);
                last_time = *time;
            }
            let first_frame_dur = Instant::now().duration_since(last_time);
            println!("  - Event Loop to First Frame: {:.2?}", first_frame_dur);
        }
    }
}
