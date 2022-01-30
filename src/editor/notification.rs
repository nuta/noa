use crate::theme::ThemeKey;

pub enum Notification {
    Info(String),
    Warn(String),
    Error(anyhow::Error),
}

pub struct NotificationManager {
    notification: Option<Notification>,
}

impl NotificationManager {
    pub fn new() -> NotificationManager {
        NotificationManager { notification: None }
    }

    pub fn last_notification(&self) -> Option<&Notification> {
        self.notification.as_ref()
    }

    pub fn last_notification_as_str(&self) -> Option<(ThemeKey, String)> {
        self.last_notification().map(|noti| match noti {
            Notification::Info(message) => (ThemeKey::InfoNotification, message.clone()),
            Notification::Warn(message) => (ThemeKey::WarnNotification, message.clone()),
            Notification::Error(err) => (ThemeKey::ErrorNotification, format!("{}", err)),
        })
    }

    pub fn info<T: Into<String>>(&mut self, message: T) {
        let message = message.into();
        info!("notification: {}", message);
        self.notification = Some(Notification::Info(message));
    }

    pub fn warn<T: Into<String>>(&mut self, message: T) {
        let message = message.into();
        warn!("notification: {}", message);
        self.notification = Some(Notification::Warn(message));
    }

    pub fn error(&mut self, err: anyhow::Error) {
        error!("notification: {}", err);
        self.notification = Some(Notification::Error(err));
    }

    pub fn maybe_error<T>(&mut self, result: anyhow::Result<T>) {
        if let Err(err) = result {
            self.error(err);
        }
    }
}
