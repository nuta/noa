pub enum Notification {
    Info(String),
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

    pub fn last_notification_as_str(&self) -> Option<String> {
        self.last_notification().map(|noti| match noti {
            Notification::Info(message) => message.clone(),
            Notification::Error(err) => {
                format!("")
            }
        })
    }

    pub fn info<T: Into<String>>(&mut self, message: T) {
        self.notification = Some(Notification::Info(message.into()));
    }

    pub fn error(&mut self, err: anyhow::Error) {
        self.notification = Some(Notification::Error(err));
    }

    pub fn maybe_error<T>(&mut self, result: anyhow::Result<T>) {
        if let Err(err) = result {
            self.error(err);
        }
    }
}
