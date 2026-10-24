//! Context-affine workers with bounded queues and cooperative cancellation.
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn canceled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
pub struct Job<T> {
    pub id: u64,
    pub epoch: u64,
    pub generation: u64,
    pub interactive: bool,
    pub cancel: Cancellation,
    pub payload: T,
    pub submitted: Instant,
}
pub struct Completion<R> {
    pub id: u64,
    pub epoch: u64,
    pub generation: u64,
    pub result: R,
    pub queue_time: Duration,
    pub run_time: Duration,
}
struct Queue<T> {
    jobs: VecDeque<Job<T>>,
    stopped: bool,
}
type SharedQueue<T> = Arc<(Mutex<Queue<T>>, Condvar)>;

pub struct Scheduler<T, R> {
    queues: Vec<SharedQueue<T>>,
    ready: mpsc::Receiver<Completion<R>>,
    capacity: usize,
}
impl<T: Send + 'static, R: Send + 'static> Scheduler<T, R> {
    /// The factory executes on its owner worker; its executor need not be Send.
    pub fn new<F, E>(
        workers: usize,
        capacity: usize,
        factory: F,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self
    where
        F: Fn() -> E + Send + Sync + 'static,
        E: FnMut(T, &Cancellation) -> R + 'static,
    {
        let (send, ready) = mpsc::sync_channel(capacity);
        let factory = Arc::new(factory);
        let queues = (0..workers.max(1))
            .map(|index| {
                let queue = Arc::new((
                    Mutex::new(Queue {
                        jobs: VecDeque::<Job<T>>::new(),
                        stopped: false,
                    }),
                    Condvar::new(),
                ));
                let (owned, send, factory, wake) =
                    (queue.clone(), send.clone(), factory.clone(), wake.clone());
                thread::Builder::new()
                    .name(format!("qs-pricing-{index}"))
                    .spawn(move || {
                        let mut execute = factory();
                        let mut interactive_streak = 0;
                        loop {
                            let job = {
                                let (lock, available) = &*owned;
                                let mut queue = lock.lock().unwrap();
                                while queue.jobs.is_empty() && !queue.stopped {
                                    queue = available.wait(queue).unwrap();
                                }
                                if queue.stopped {
                                    break;
                                }
                                // Favor interactive work, but service a worksheet job every eighth turn.
                                let position = if interactive_streak < 7 {
                                    queue.jobs.iter().position(|j| j.interactive).unwrap_or(0)
                                } else {
                                    0
                                };
                                let job = queue.jobs.remove(position).unwrap();
                                interactive_streak = if job.interactive {
                                    interactive_streak + 1
                                } else {
                                    0
                                };
                                job
                            };
                            if job.cancel.canceled() {
                                continue;
                            }
                            let began = Instant::now();
                            let result = execute(job.payload, &job.cancel);
                            if job.cancel.canceled() {
                                continue;
                            }
                            let completion = Completion {
                                id: job.id,
                                epoch: job.epoch,
                                generation: job.generation,
                                result,
                                queue_time: began.duration_since(job.submitted),
                                run_time: began.elapsed(),
                            };
                            if send.send(completion).is_err() {
                                break;
                            }
                            wake();
                        }
                    })
                    .expect("start pricing worker");
                queue
            })
            .collect();
        Self {
            queues,
            ready,
            capacity,
        }
    }
    /// Never waits for a worker. Canceled queued jobs are removed immediately.
    pub fn submit(&self, affinity: u64, job: Job<T>) -> Result<(), Job<T>> {
        let (lock, wake) = &*self.queues[affinity as usize % self.queues.len()];
        let mut queue = lock.lock().unwrap();
        queue.jobs.retain(|j| !j.cancel.canceled());
        if queue.stopped || queue.jobs.len() >= self.capacity {
            return Err(job);
        }
        queue.jobs.push_back(job);
        wake.notify_one();
        Ok(())
    }
    pub fn take(&self) -> Option<Completion<R>> {
        self.ready.try_recv().ok()
    }
    pub fn workers(&self) -> usize {
        self.queues.len()
    }
    /// Owner lifecycle commands precede queued prices and cannot be rejected by
    /// a full pricing queue. There is at most one command per retiring workbook.
    pub fn submit_control(&self, affinity: u64, job: Job<T>) {
        let (lock, wake) = &*self.queues[affinity as usize % self.queues.len()];
        let mut queue = lock.lock().unwrap();
        if !queue.stopped {
            queue.jobs.retain(|j| !j.cancel.canceled());
            queue.jobs.push_front(job);
            wake.notify_one();
        }
    }
}
impl<T, R> Drop for Scheduler<T, R> {
    fn drop(&mut self) {
        for queue in &self.queues {
            let (lock, wake) = &**queue;
            let mut queue = lock.lock().unwrap();
            queue.stopped = true;
            for job in queue.jobs.drain(..) {
                job.cancel.cancel();
            }
            wake.notify_all();
        }
        // Dropping the completion receiver also releases workers publishing into a full queue.
        // No join on Excel's UI thread. The native bridge pins the executing module.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_local_rc_affinity_backpressure_and_cancel() {
        let scheduler = Scheduler::new(
            2,
            8,
            || {
                let state = std::rc::Rc::new(std::cell::Cell::new(0));
                move |amount, _: &Cancellation| {
                    state.set(state.get() + amount);
                    (thread::current().id(), state.get())
                }
            },
            Arc::new(|| {}),
        );
        let canceled = Cancellation::default();
        canceled.cancel();
        for (id, cancel) in [
            (1, Cancellation::default()),
            (2, canceled),
            (3, Cancellation::default()),
        ] {
            assert!(scheduler
                .submit(
                    9,
                    Job {
                        id,
                        epoch: 4,
                        generation: 2,
                        interactive: false,
                        cancel,
                        payload: 1,
                        submitted: Instant::now()
                    }
                )
                .is_ok());
        }
        let mut results = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while results.len() < 2 && Instant::now() < deadline {
            if let Some(value) = scheduler.take() {
                results.push(value);
            } else {
                thread::yield_now();
            }
        }
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].result.0, results[1].result.0);
        assert_ne!(results[0].result.0, thread::current().id());
        assert_eq!((results[0].result.1, results[1].result.1), (1, 2));
        assert_eq!((results[1].epoch, results[1].generation), (4, 2));
    }
}
