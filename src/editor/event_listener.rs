use noa_common::oops::OopsExt;
use tokio::sync::watch;

pub struct EventProducer(watch::Sender<()>);

impl EventProducer {
    pub fn notify_all(&self) {
        self.0.send(()).oops();
    }
}

#[derive(Clone)]
pub struct EventListener(watch::Receiver<()>);

impl EventListener {
    pub async fn notified(&mut self) {
        self.0.changed().await.oops();
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
