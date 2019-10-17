use std::time::SystemTime;

#[cfg(debug_assertions)]
pub fn report_exec_time(title: &'static str, func: impl FnOnce() -> ()) {
    let started_at = SystemTime::now();
    func();
    let duration = started_at.elapsed().unwrap();
    trace!("{} took {}ms", title, duration.as_millis());
}

#[cfg(not(debug_assertions))]
#[inline(always)]
pub fn report_exec_time(_title: &'static str, func: impl FnOnce() -> ()) {
    func();
}
