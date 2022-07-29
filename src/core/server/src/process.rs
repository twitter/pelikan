use crate::*;
use std::thread::JoinHandle;

pub struct ProcessBuilder<Parser, Request, Response, Storage> {
    admin: AdminBuilder,
    listener: ListenerBuilder,
    log_drain: Box<dyn Drain>,
    workers: WorkersBuilder<Parser, Request, Response, Storage>,
}

impl<Parser, Request, Response, Storage> ProcessBuilder<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Send,
    Response: 'static + Compose + Send,
    Storage: 'static + Execute<Request, Response> + EntryStore + Send,
{
    pub fn new<T: AdminConfig + ServerConfig + TlsConfig + WorkerConfig>(
        config: &T,
        log_drain: Box<dyn Drain>,
        parser: Parser,
        storage: Storage,
    ) -> Result<Self> {
        let admin = AdminBuilder::new(config)?;
        let listener = ListenerBuilder::new(config)?;
        let workers = WorkersBuilder::new(config, parser, storage)?;

        Ok(Self {
            admin,
            listener,
            log_drain,
            workers,
        })
    }

    pub fn version(mut self, version: &str) -> Self {
        self.admin.version(version);
        self
    }

    pub fn spawn(self) -> Process {
        let mut thread_wakers = vec![self.listener.waker()];
        thread_wakers.extend_from_slice(&self.workers.wakers());

        // channel for the parent `Process` to send `Signal`s to the admin thread
        let (signal_tx, signal_rx) = bounded(QUEUE_CAPACITY);

        // queues for the `Admin` to send `Signal`s to all sibling threads
        let (mut signal_queue_tx, mut signal_queue_rx) =
            Queues::new(vec![self.admin.waker()], thread_wakers, QUEUE_CAPACITY);

        // queues for the `Listener` to send `Session`s to the worker threads
        let (mut listener_session_queues, worker_session_queues) = Queues::new(
            vec![self.listener.waker()],
            self.workers.worker_wakers(),
            QUEUE_CAPACITY,
        );

        let mut admin = self
            .admin
            .build(self.log_drain, signal_rx, signal_queue_tx.remove(0));

        let mut listener = self
            .listener
            .build(signal_queue_rx.remove(0), listener_session_queues.remove(0));

        let workers = self.workers.build(worker_session_queues, signal_queue_rx);

        let admin = std::thread::Builder::new()
            .name(format!("{}_admin", THREAD_PREFIX))
            .spawn(move || admin.run())
            .unwrap();

        let listener = std::thread::Builder::new()
            .name(format!("{}_listener", THREAD_PREFIX))
            .spawn(move || listener.run())
            .unwrap();

        let workers = workers.spawn();

        Process {
            admin,
            listener,
            signal_tx,
            workers,
        }
    }
}

pub struct Process {
    admin: JoinHandle<()>,
    listener: JoinHandle<()>,
    signal_tx: Sender<Signal>,
    workers: Vec<JoinHandle<()>>,
}

impl Process {
    /// Attempts to gracefully shutdown the `Process` by sending a shutdown to
    /// each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(self) {
        // this sends a shutdown to the admin thread, which will broadcast the
        // signal to all sibling threads in the process
        if self.signal_tx.try_send(Signal::Shutdown).is_err() {
            fatal!("error sending shutdown signal to thread");
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        for thread in self.workers {
            let _ = thread.join();
        }
        let _ = self.listener.join();
        let _ = self.admin.join();
    }
}
