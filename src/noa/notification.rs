use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use arc_swap::ArcSwap;
use once_cell::sync::Lazy;

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
    notification: ArcSwap<Option<Notification>>,
}

impl NotificationManager {
    fn new() -> NotificationManager {
        NotificationManager {
            notification: ArcSwap::from_pointee(None),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.notification.load().is_none()
    }

    pub fn last_notification(&self) -> arc_swap::Guard<Arc<Option<Notification>>> {
        self.notification.load()
    }

    pub fn clear(&self) {
        self.notification.store(Arc::new(None));
    }

    pub fn notify(&self, noti: Notification) {
        info!("notification: {:?}", noti);

        if PRINT_TO_STDOUT.load(Ordering::SeqCst) {
            match &noti {
                Notification::Error(msg) => eprintln!("{}", msg),
                Notification::Warn(msg) => eprintln!("{}", msg),
                Notification::Info(msg) => eprintln!("{}", msg),
            }
        }

        self.notification.store(Arc::new(Some(noti)));
    }
}

#[macro_export]
macro_rules! notify_info {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Info(format!($($arg)+));
        notification_manager().notify(noti);
    }}
}

#[macro_export]
macro_rules! notify_warn {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Warn(format!($($arg)+));
        notification_manager().notify(noti);
    }}
}

#[macro_export]
macro_rules! notify_error {
    ($($arg:tt)+) => {{
        use $crate::notification::{Notification, notification_manager};
        let noti = Notification::Error(format!($($arg)+));
        notification_manager().notify(noti);
    }}
}

#[macro_export]
macro_rules! notify_anyhow_error {
    ($err:expr) => {{
        use $crate::notification::{notification_manager, Notification};
        let noti = Notification::from($err);
        notification_manager().notify(noti);
    }};
}

static NOTIFICATIONS: Lazy<NotificationManager> = Lazy::new(NotificationManager::new);
static PRINT_TO_STDOUT: AtomicBool = AtomicBool::new(false);

pub fn notification_manager() -> &'static Lazy<NotificationManager> {
    &NOTIFICATIONS
}

pub fn set_stdout_mode(enable: bool) {
    PRINT_TO_STDOUT.store(enable, Ordering::SeqCst);
}
