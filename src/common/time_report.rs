use std::time::Instant;

pub struct TimeReport {
    title: String,
    started_at: Instant,
}

impl TimeReport {
    pub fn new(title: &str) -> TimeReport {
        let title = title.to_string();
        TimeReport {
            title,
            started_at: Instant::now(),
        }
    }

    pub fn report(self) {
        trace!(
            "time_report: {} took {:?}",
            self.title,
            self.started_at.elapsed()
        );
    }
}
