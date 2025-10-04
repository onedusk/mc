use indicatif::{ProgressBar, ProgressStyle};

pub trait Progress: Send + Sync {
    fn increment(&self, delta: u64);
    fn set_message(&self, msg: &str);
    fn finish(&self);
}

pub struct ProgressReporter {
    bar: ProgressBar,
}

impl ProgressReporter {
    pub fn new(total: u64) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        Self { bar }
    }
}

impl Progress for ProgressReporter {
    fn increment(&self, delta: u64) {
        self.bar.inc(delta);
    }

    fn set_message(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    fn finish(&self) {
        self.bar.finish_with_message("Complete");
    }
}

pub struct NoOpProgress;

impl Progress for NoOpProgress {
    fn increment(&self, _: u64) {}
    fn set_message(&self, _: &str) {}
    fn finish(&self) {}
}