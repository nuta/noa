use std::thread;
use std::sync::mpsc::{channel, Sender};
use crate::editor::EventQueue;

const NUM_WORKER_THREADS: usize = 4;

pub trait Job {
    fn execute(&mut self, event_queue: &EventQueue);
}

pub struct Worker {
    queues: Vec<Sender<Box<dyn Job + Send>>>,
    roundrobin_index: usize,
}

impl Worker {
    pub fn new(event_queue: EventQueue) -> Worker {
        let mut queues = Vec::new();
        for _ in 0..NUM_WORKER_THREADS {
            let (tx, rx) = channel::<Box<dyn Job + Send>>();
            let eq = event_queue.clone();
            queues.push(tx);
            thread::spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    job.execute(&eq);
                }
            });
        }

        Worker {
            queues,
            roundrobin_index: 0,
        }
    }

    pub fn request(&mut self, job: Box<dyn Job + Send>) {
        let index = self.roundrobin_index % self.queues.len();
        self.roundrobin_index = self.roundrobin_index.wrapping_add(1);
        self.queues[index].send(job).unwrap();
    }
}
