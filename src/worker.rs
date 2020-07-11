use std::thread;
use std::sync::mpsc::{channel, Sender};
use crate::editor::{Event, EventQueue};

const NUM_WORKER_THREADS: usize = 4;

pub trait Job {
    fn execute(&mut self, event_queue: &EventQueue);
}

pub struct Request {
    job: Box<dyn Job + Send>,
}

pub struct Worker {
    queues: Vec<Sender<Request>>,
    roundrobin_index: usize,
}

impl Worker {
    pub fn new(event_queue: EventQueue) -> Worker {
        let mut queues = Vec::new();
        for _ in 0..NUM_WORKER_THREADS {
            let (tx, rx) = channel::<Request>();
            let mut eq = event_queue.clone();
            queues.push(tx);
            thread::spawn(move || {
                while let Ok(mut req) = rx.recv() {
                    req.job.execute(&mut eq);
                }
            });
        }

        Worker {
            queues,
            roundrobin_index: 0,
        }
    }

    pub fn request(&mut self, req: Request) {
        let index = self.roundrobin_index % self.queues.len();
        self.roundrobin_index = self.roundrobin_index.wrapping_add(1);
        self.queues[index].send(req).unwrap();
    }
}
