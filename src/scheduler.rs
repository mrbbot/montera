use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

// Job's must also implement Send so they can be sent between worker threads safely
pub trait Job: Send {
    fn process(&self);
}

pub trait Scheduler {
    fn schedule(&self, job: Box<dyn Job>); // Dynamic dispatch
}

pub struct WorkerScheduler {
    sender: Sender<Box<dyn Job>>,
    receiver: Arc<Mutex<Receiver<Box<dyn Job>>>>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerScheduler {
    pub fn new(workers: usize) -> Self {
        // Create a multi-producer single-consumer channel with an *infinite* buffer,
        // we're basically turning this into a single-producer multi-consumer channel
        let (sender, receiver) = channel();
        // We need to mediate multi-threaded access to the receiver
        let receiver = Arc::new(Mutex::new(receiver));
        // Hold on to thread handles so threads aren't detached after spawn
        let handles = vec![];

        // Spawn n worker threads
        let mut schd = Self {
            sender,
            receiver,
            handles,
        };
        debug!("Starting {} workers...", workers);
        for _ in 0..workers {
            schd.spawn_worker()
        }
        schd
    }

    fn spawn_worker(&mut self) {
        // Create a copy of the queue for this thread
        let thread_receiver = Arc::clone(&self.receiver);
        let handle = thread::spawn(move || {
            loop {
                // lock() only fails if the thread previously holding the mutex panicked
                let receiver_guard = thread_receiver.lock().unwrap();
                let job = receiver_guard.recv();
                // Explicitly release mutex here and allow another worker to access the queue
                drop(receiver_guard);
                match job {
                    Ok(job) => job.process(),
                    // recv() fails if all senders dropped. In our case, the only sender is the
                    // `self.sender` so this will fail when the JobQueue is dropped. It would be
                    // impossible to enqueue anymore work after this, so this is what we want.
                    Err(_) => break,
                }
            }
        });
        // Hold on to thread handle so thread isn't detached
        self.handles.push(handle);
    }
}

impl Scheduler for WorkerScheduler {
    fn schedule(&self, job: Box<dyn Job>) {
        self.sender.send(job).unwrap();
    }
}

pub struct SerialScheduler;

impl Scheduler for SerialScheduler {
    fn schedule(&self, job: Box<dyn Job>) {
        job.process();
    }
}
