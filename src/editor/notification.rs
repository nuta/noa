use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::theme::ThemeKey;

#[derive(Debug)]
pub enum Notification {
    Info(String),
    Warn(String),
    Error(String),
}

impl From<anyhow::Error> for Notification {
    fn from(err: anyhow::Error) -> Notification {
        Notification::Error(format!("{}", err))
    }
}

pub struct NotificationManager {
    notification: Option<Notification>,
}

impl NotificationManager {
    fn new() -> NotificationManager {
        NotificationManager { notification: None }
    }

    pub fn last_notification(&self) -> Option<&Notification> {
        self.notification.as_ref()
    }

    // TODO: Stop cloning string.
    pub fn last_notification_as_str(&self) -> Option<(&'static str, String)> {
        self.last_notification().map(|noti| match noti {
            Notification::Info(message) => ("notification.info", message.clone()),
            Notification::Warn(message) => ("notification.warn", message.clone()),
            Notification::Error(err) => ("notification.error", err.clone()),
        })
    }

    pub fn push(&mut self, noti: Notification) {
        info!("notification: {:?}", noti);
        self.notification = Some(noti);
    }
}

#[macro_export]
macro_rules! notify_info {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Info(format!($($arg)+));
        notification_manager().lock().push(noti);
    }}
}

#[macro_export]
macro_rules! notify_warn {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Warn(format!($($arg)+));
        notification_manager().lock().push(noti);
    }}
}

#[macro_export]
macro_rules! notify_error {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Error(format!($($arg)+));
        notification_manager().lock().push(noti);
    }}
}

#[macro_export]
macro_rules! notify_anyhow_error {
    ($err:expr) => {{
        use $crate::notification::{notification_manager, Notification};
        let noti = Notification::from($err);
        notification_manager().lock().push(noti);
    }};
}

static NOTIFICATIONS: Lazy<Mutex<NotificationManager>> =
    Lazy::new(|| Mutex::new(NotificationManager::new()));

pub fn notification_manager() -> &'static Lazy<Mutex<NotificationManager>> {
    &NOTIFICATIONS
}
