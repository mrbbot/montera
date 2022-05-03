use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

/// Job that can be scheduled using a [`Scheduler`]. `Job`'s must also implement `Send` so they can be
/// sent between worker threads safely. Results should be returned using channels.
pub trait Job: Send {
    fn process(&self);
}

/// Job scheduler.
pub trait Scheduler {
    fn schedule(&self, job: Box<dyn Job>); // `dyn` is dynamic dispatch
}

/// Schedules jobs across worker threads, executing jobs in parallel.
///
/// Uses a MPSC channel to send jobs to workers. Access to the receiving side is mediated using a
/// mutual-exclusion lock.
#[cfg(feature = "parallel_scheduler")]
pub struct WorkerScheduler {
    sender: Sender<Box<dyn Job>>,
    receiver: Arc<Mutex<Receiver<Box<dyn Job>>>>,
    handles: Vec<JoinHandle<()>>,
}

#[cfg(feature = "parallel_scheduler")]
impl WorkerScheduler {
    /// Constructs a new scheduler using `workers` worker threads.
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

    /// Creates a new worker thread. This will be called `workers` times by [`WorkerScheduler::new`].
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
        // Send the job on the channel to any receiving worker thread
        self.sender.send(job).unwrap();
    }
}

/// Schedules jobs immediately on the current thread, executing jobs in serial.
#[cfg(any(not(feature = "parallel_scheduler"), test))]
pub struct SerialScheduler;

#[cfg(any(not(feature = "parallel_scheduler"), test))]
impl Scheduler for SerialScheduler {
    fn schedule(&self, job: Box<dyn Job>) {
        job.process();
    }
}

#[cfg(test)]
mod tests {
    use crate::scheduler::{Job, SerialScheduler, WorkerScheduler};
    use crate::Scheduler;
    use std::sync::mpsc::{channel, Sender};
    use std::thread::sleep;
    use std::time::Duration;

    struct TestJob {
        result: u64,
        result_tx: Sender<u64>,
    }

    impl TestJob {
        pub fn new(result: u64, result_tx: &Sender<u64>) -> Self {
            Self {
                result,
                result_tx: result_tx.clone(),
            }
        }
    }

    impl Job for TestJob {
        fn process(&self) {
            sleep(Duration::from_millis(self.result));
            self.result_tx.send(self.result).unwrap();
        }
    }

    fn test_scheduler(schd: Box<dyn Scheduler>) {
        let (result_tx, result_rx) = channel();
        let job1 = TestJob::new(0, &result_tx);
        let job2 = TestJob::new(250, &result_tx);
        let job3 = TestJob::new(500, &result_tx);
        drop(result_tx);

        schd.schedule(Box::new(job1));
        schd.schedule(Box::new(job2));
        schd.schedule(Box::new(job3));

        let results = result_rx.into_iter().collect::<Vec<_>>();
        assert_eq!(results, [0, 250, 500]);
    }

    #[test]
    fn worker_scheduler() {
        test_scheduler(Box::new(WorkerScheduler::new(4)))
    }

    #[test]
    fn serial_scheduler() {
        test_scheduler(Box::new(SerialScheduler {}))
    }
}
