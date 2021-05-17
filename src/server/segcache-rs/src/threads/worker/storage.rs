use crate::common::Queue;
use crate::common::Sender;
use crate::common::Signal;
use crate::common::*;
use crate::Compose;
use crate::Execute;
use config::WorkerConfig;
use metrics::Stat;
use mio::Events;
use mio::Poll;
use mio::Token;
use mio::Waker;
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

/// A `Storage` thread is used in a multi-worker configuration. It owns the
/// cache contents and operates on message queues for each worker thread, taking
/// fully parsed requests, processing them, and writing the responses directly
/// into the session write buffers.
pub struct StorageWorker<Storage, Response, Request> {
    config: Arc<WorkerConfig>,
    poll: Poll,
    storage: Storage,
    signal_queue: Queue<Signal>,
    waker: Arc<Waker>,
    worker_queues: Vec<BiDiQueue<Response, Request>>,
}

impl<Storage, Response, Request> StorageWorker<Storage, Response, Request>
where
    Response: Compose,
    Storage: Execute<Request, Response> + crate::storage::Storage,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(config: Arc<WorkerConfig>, storage: Storage) -> Result<Self, std::io::Error> {
        let signal_queue = Queue::new(128);

        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let waker = Arc::new(Waker::new(poll.registry(), Token(usize::MAX)).unwrap());

        Ok(Self {
            config,
            poll,
            storage,
            signal_queue,
            waker,
            worker_queues: Vec::new(),
        })
    }

    /// Add a queue for a worker by providing the worker's `Waker` so that the
    /// worker can be notified of pending responses from the storage thread
    pub fn add_queue(&mut self, waker: Arc<Waker>) -> BiDiQueue<Request, Response> {
        let (to_storage, from_worker) = rtrb::RingBuffer::new(65536).split();
        let (to_worker, from_storage) = rtrb::RingBuffer::new(65536).split();

        let worker_queue = BiDiQueue {
            send: to_worker,
            recv: from_worker,
            waker,
        };

        let storage_queue = BiDiQueue {
            send: to_storage,
            recv: from_storage,
            waker: self.waker.clone(),
        };

        self.worker_queues.push(worker_queue);
        storage_queue
    }

    /// Run the storage thread in a loop, handling incoming messages from the
    /// worker threads
    pub fn run(&mut self) {
        // holds the number of workers registered
        let workers = self.worker_queues.len();

        // holds state about whether a given worker needs a deferred wake, this
        // is used to coalesce wakeups and reduce syscall load
        let mut worker_needs_wake = vec![false; workers];

        // holds state about how many messages were pending for each worker when
        // a wakeup happened
        let mut worker_pending = vec![0; workers];

        let mut events = Events::with_capacity(self.config.nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.timeout() as u64
        ));

        loop {
            increment_counter!(&Stat::StorageEventLoop);

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            if !events.is_empty() {
                // store the number of messages currently in each queue when
                // wakeup occurred
                for (id, queue) in self.worker_queues.iter_mut().enumerate() {
                    worker_pending[id] = queue.pending();
                }

                let mut empty = false;

                while !empty {
                    empty = true;
                    for id in 0..workers {
                        if worker_pending[id] > 0 {
                            if let Ok(message) = self.worker_queues[id].try_recv() {
                                increment_counter!(&Stat::ProcessReq);
                                let response = self.storage.execute(message.item);
                                let mut message = Message {
                                    item: response,
                                    token: message.token,
                                };
                                for retry in 0..QUEUE_RETRIES {
                                    if let Err(PushError::Full(m)) =
                                        self.worker_queues[id].try_send(message)
                                    {
                                        if (retry + 1) == QUEUE_RETRIES {
                                            error!("error sending message to worker");
                                        }
                                        let _ = self.worker_queues[id].wake();
                                        message = m;
                                    } else {
                                        break;
                                    }
                                }
                                worker_needs_wake[id] = true;
                            }
                            empty = false;
                            worker_pending[id] -= 1;
                        }
                    }
                }

                for (id, needs_wake) in worker_needs_wake.iter_mut().enumerate() {
                    if *needs_wake {
                        let _ = self.worker_queues[id].wake();
                        *needs_wake = false;
                    }
                }
            }

            // poll queue to receive new messages
            #[allow(clippy::never_loop)]
            while let Ok(s) = self.signal_queue.try_recv() {
                match s {
                    Signal::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    pub fn signal_sender(&self) -> Sender<Signal> {
        self.signal_queue.sender()
    }
}