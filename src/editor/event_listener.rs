use noa_common::oops::OopsExt;
use tokio::sync::watch;

pub struct EventProducer(watch::Sender<()>);

impl EventProducer {
    pub fn notify_all(&self) {
        let _ = self.0.send(());
    }
}

#[derive(Clone)]
pub struct EventListener(watch::Receiver<()>);

impl EventListener {
    pub async fn notified(&mut self) {
        let _ = self.0.changed().await;
    }
}

pub struct EventPair {
    pub producer: EventProducer,
    pub listener: EventListener,
}

pub fn event_pair() -> EventPair {
    let (tx, rx) = watch::channel(());
    EventPair {
        producer: EventProducer(tx),
        listener: EventListener(rx),
    }
}
