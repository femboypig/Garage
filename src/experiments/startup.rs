use std::time::Instant;
use std::sync::OnceLock;

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn record_start_time() {
    let _ = START_TIME.set(Instant::now());
}

pub fn report_startup_complete() {
    if let Some(start) = START_TIME.get() {
        let duration = start.elapsed();
        println!("🚀 Startup complete! Time elapsed: {:.2?}", duration);
    }
}
