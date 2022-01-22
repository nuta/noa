use std::time::Instant;

use once_cell::sync::OnceCell;

static ENABLED: OnceCell<bool> = OnceCell::new();

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
        if !ENABLED
            .get_or_init(|| std::option_env!("TIME_REPORT").is_some() || cfg!(debug_assertions))
        {
            return;
        }

        info!(
            "time_report: {} took {:?}",
            self.title,
            self.started_at.elapsed()
        );
    }
}
