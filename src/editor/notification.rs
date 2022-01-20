pub enum Notification {
    Error(anyhow::Error),
}

pub struct NotificationManager {
    notification: Option<Notification>,
}

impl NotificationManager {
    pub fn new() -> NotificationManager {
        NotificationManager { notification: None }
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
