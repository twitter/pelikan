use crate::*;
use std::thread::JoinHandle;
// use std::time::Instant;

pub struct SingleWorker<Parser, Request, Response, Storage> {
    nevent: usize,
    parser: Parser,
    poll: Poll,
    session_queue: Queues<Session, Session>,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    signal_queue: Queues<(), Signal>,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Waker>,
}

fn map_err(e: std::io::Error) -> Result<()> {
	match e.kind() {
        ErrorKind::WouldBlock => {
            Ok(())
        }
        _ => Err(e),
    }
}

impl<Parser, Request, Response, Storage> SingleWorker<Parser, Request, Response, Storage>
where
	Parser: Parse<Request> + Clone,
	Response: Compose,
    Storage: EntryStore + Execute<Request, Response>,
{
	/// Return the `Session` to the `Listener` to handle flush/close
    fn close(&mut self, token: Token) {
    	if self.sessions.contains(token.0) {
    		let mut session = self.sessions.remove(token.0).into_inner();
	        let _ = session.deregister(self.poll.registry());
	        let _ = self.session_queue.try_send_any(session);
	        let _ = self.session_queue.wake();
    	}
    }

    /// Handle up to one request for a session
    fn read(&mut self, token: Token) -> Result<()> {
    	let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // fill the session
        match session.fill() {
        	Ok(0) => {
        		Err(Error::new(ErrorKind::Other, "client hangup"))
        	}
        	r => r,
        }?;

        // process up to one request
        match session.receive() {
        	Ok(request) => {
        		let response = self.storage.execute(&request);
        		match session.send(response) {
        			Ok(_) => {
        				if session.write_pending() > 0 {
        					match session.flush() {
					        	Ok(_) => Ok(()),
					        	Err(e) => map_err(e),
					        }?;
        				}

        				if session.write_pending() > 0 && session.reregister(self.poll.registry(), token, session.interest()).is_err() {
    						Err(Error::new(ErrorKind::Other, "failed to reregister"))
    					} else {
    						Ok(())
    					}
        			},
        			Err(e) => map_err(e),
        		}
        	}
        	Err(e) => map_err(e),
        }
    }

    fn write(&mut self, token: Token) -> Result<()> {
    	let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        match session.flush() {
        	Ok(_) => Ok(()),
        	Err(e) => map_err(e),
        }
    }

    /// Run the worker in a loop, handling new events.
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);

        loop {
            // WORKER_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            // let timestamp = Instant::now();

            // let count = events.iter().count();
            // WORKER_EVENT_TOTAL.add(count as _);
            // if count == self.nevent {
            // WORKER_EVENT_MAX_REACHED.increment();
            // } else {
            // WORKER_EVENT_DEPTH.increment(timestamp, count as _, 1);
            // }

            // process all events
            for event in events.iter() {
            	let token = event.token();

                match token {
                    WAKER_TOKEN => {
                    	// handle up to one new session
                        if let Some(mut session) = self.session_queue.try_recv().map(|v| v.into_inner()) {
				            let s = self.sessions.vacant_entry();
				            if session.register(self.poll.registry(), Token(s.key()), session.interest()).is_ok() {
				            	s.insert(ServerSession::new(session, self.parser.clone()));
				            } else {
				            	let _ = self.session_queue.try_send_any(session);
				            }
				            
				            // trigger a wake-up in case there are more sessions
				            let _ = self.waker.wake();
				        }

                        // check if we received any signals from the admin thread
                        while let Some(signal) = self.signal_queue.try_recv() {
                            match signal.into_inner() {
                                Signal::FlushAll => {
                                    warn!("received flush_all");
                                    self.storage.clear();
                                }
                                Signal::Shutdown => {
                                    // if we received a shutdown, we can return
                                    // and stop processing events
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                    	if event.is_error() || event.is_writable() && self.write(token).is_err() || event.is_readable() && self.read(token).is_err() {
				            self.close(token);
				        }
                    }
                }
            }
        }
    }
}

pub struct SingleWorkerBuilder<Parser, Request, Response, Storage> {
    parser: Parser,
    poll: Poll,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl<Parser, Request, Response, Storage> SingleWorkerBuilder<Parser, Request, Response, Storage> {
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser, storage: Storage) -> Result<Self> {
        let config = config.worker();

        let poll = Poll::new()?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        // let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        Ok(Self {
            parser,
            poll,
            sessions: Slab::new(),
            storage,
            timeout,
            waker,
        })
    }

    pub fn build(
        self,
        session_queue: Queues<Session, Session>,
        signal_queue: Queues<(), Signal>,
    ) -> SingleWorker<Parser, Request, Response, Storage> {
        SingleWorker {
            nevent: 1024,
            parser: self.parser,
            poll: self.poll,
            session_queue,
            sessions: self.sessions,
            signal_queue,
            storage: self.storage,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

pub struct MultiWorker<Parser, Request, Response> {
    data_queue: Queues<(Request, Token), (Request, Response, Token)>,
    nevent: usize,
    parser: Parser,
    poll: Poll,
    session_queue: Queues<Session, Session>,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    signal_queue: Queues<(), Signal>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl<Parser, Request, Response> MultiWorker<Parser, Request, Response>
where
    Parser: Parse<Request> + Clone,
    Response: Compose,
{
    /// Return the `Session` to the `Listener` to handle flush/close
    fn close(&mut self, token: Token) {
        if self.sessions.contains(token.0) {
    		let mut session = self.sessions.remove(token.0).into_inner();
	        let _ = session.deregister(self.poll.registry());
	        let _ = self.session_queue.try_send_any(session);
	        let _ = self.session_queue.wake();
    	}
    }

    /// Handle up to one request for a session
    fn read(&mut self, token: Token) -> Result<()> {
    	let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // fill the session
        match session.fill() {
        	Ok(0) => {
        		Err(Error::new(ErrorKind::Other, "client hangup"))
        	}
        	r => r,
        }?;

        // process up to one request
        match session.receive() {
        	Ok(request) => {
        		self.data_queue.try_send_to(0, (request, token)).map_err(|_| Error::new(ErrorKind::Other, "data queue is full"))
        	}
        	Err(e) => {
        		map_err(e)
        	}
        }
    }

    /// Handle write by flushing the session
    fn write(&mut self, token: Token) -> Result<()> {
    	let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        match session.flush() {
        	Ok(_) => Ok(()),
        	Err(e) => map_err(e),
        }
    }

    /// Run the worker in a loop, handling new events.
    pub fn run(&mut self) {
        // these are buffers which are re-used in each loop iteration to receive
        // events and queue messages
        let mut events = Events::with_capacity(self.nevent);
        let mut messages = Vec::with_capacity(QUEUE_CAPACITY);
        // let mut sessions = Vec::with_capacity(QUEUE_CAPACITY);

        loop {
            // WORKER_EVENT_LOOP.increment();

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            // let timestamp = Instant::now();

            // let count = events.iter().count();
            // WORKER_EVENT_TOTAL.add(count as _);
            // if count == self.nevent {
            //     WORKER_EVENT_MAX_REACHED.increment();
            // } else {
            //     WORKER_EVENT_DEPTH.increment(timestamp, count as _, 1);
            // }

            // process all events
            for event in events.iter() {
            	let token = event.token();
                match token {
                    WAKER_TOKEN => {
                    	// handle up to one new session
                        if let Some(mut session) = self.session_queue.try_recv().map(|v| v.into_inner()) {
				            let s = self.sessions.vacant_entry();
				            if session.register(self.poll.registry(), Token(s.key()), session.interest()).is_ok() {
				            	s.insert(ServerSession::new(session, self.parser.clone()));
				            } else {
				            	let _ = self.session_queue.try_send_any(session);
				            }
				            
				            // trigger a wake-up in case there are more sessions
				            let _ = self.waker.wake();
				        }

				        // handle all pending messages on the data queue
				        self.data_queue.try_recv_all(&mut messages);
				        for (_request, response, token) in messages.drain(..).map(|v| v.into_inner()) {
				            if let Some(session) = self.sessions.get_mut(token.0) {
				                if session.send(response).is_err() || session.write_pending() > 0 && session.reregister(self.poll.registry(), token, session.interest()).is_err() {
				                	self.close(token);
				                }
				            }
				        }

                        // check if we received any signals from the admin thread
                        while let Some(signal) =
                            self.signal_queue.try_recv().map(|v| v.into_inner())
                        {
                            match signal {
                                Signal::FlushAll => {}
                                Signal::Shutdown => {
                                    // if we received a shutdown, we can return
                                    // and stop processing events
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                    	// handle each session event
                        if event.is_error() || event.is_writable() && self.write(token).is_err() || event.is_readable() && self.read(token).is_err() {
				            self.close(token);
				        }
                    }
                }
            }

            // wakes the storage thread if necessary
            let _ = self.data_queue.wake();
        }
    }
}

pub struct MultiWorkerBuilder<Parser, Request, Response> {
    nevent: usize,
    parser: Parser,
    poll: Poll,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl<Parser, Request, Response> MultiWorkerBuilder<Parser, Request, Response> {
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser) -> Result<Self> {
        let config = config.worker();

        let poll = Poll::new()?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        Ok(Self {
            nevent,
            parser,
            poll,
            sessions: Slab::new(),
            timeout,
            waker,
        })
    }

    pub fn build(
        self,
        data_queue: Queues<(Request, Token), (Request, Response, Token)>,
        session_queue: Queues<Session, Session>,
        signal_queue: Queues<(), Signal>,
    ) -> MultiWorker<Parser, Request, Response> {
        MultiWorker {
            data_queue,
            nevent: self.nevent,
            parser: self.parser,
            poll: self.poll,
            session_queue,
            sessions: self.sessions,
            signal_queue,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

pub struct StorageWorker<Request, Response, Storage> {
    data_queue: Queues<(Request, Response, Token), (Request, Token)>,
    nevent: usize,
    poll: Poll,
    signal_queue: Queues<(), Signal>,
    storage: Storage,
    timeout: Duration,
    #[allow(dead_code)]
    waker: Arc<Waker>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Request, Response, Storage> StorageWorker<Request, Response, Storage>
where
    Storage: Execute<Request, Response> + EntryStore,
    Response: Compose,
{
    /// Run the `StorageWorker` in a loop, handling new session events.
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);
        let mut messages = Vec::with_capacity(1024);

        loop {
            // STORAGE_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            // let timestamp = Instant::now();

            if !events.is_empty() {
                trace!("handling events");

                self.data_queue.try_recv_all(&mut messages);

                // STORAGE_QUEUE_DEPTH.increment(timestamp, requests.len() as _, 1);

                for message in messages.drain(..) {
                    let sender = message.sender();
                    let (request, token) = message.into_inner();
                    trace!("handling request from worker: {}", sender);
                    let response = self.storage.execute(&request);
                    let mut message = (request, response, token);
                    for retry in 0..QUEUE_RETRIES {
                        if let Err(m) = self.data_queue.try_send_to(sender, message) {
                            if (retry + 1) == QUEUE_RETRIES {
                                error!("error sending message to worker");
                            }
                            // wake workers immediately
                            let _ = self.data_queue.wake();
                            message = m;
                        } else {
                            break;
                        }
                    }
                }

                let _ = self.data_queue.wake();

                // check if we received any signals from the admin thread
                while let Some(s) = self.signal_queue.try_recv().map(|v| v.into_inner()) {
                    match s {
                        Signal::FlushAll => {
                            warn!("received flush_all");
                            self.storage.clear();
                        }
                        Signal::Shutdown => {
                            // if we received a shutdown, we can return and stop
                            // processing events

                            // TODO(bmartin): graceful shutdown would occur here
                            // when we add persistence

                            return;
                        }
                    }
                }
            }
        }
    }
}

pub struct StorageWorkerBuilder<Request, Response, Storage> {
    nevent: usize,
    poll: Poll,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Waker>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Request, Response, Storage> StorageWorkerBuilder<Request, Response, Storage> {
    pub fn new<T: WorkerConfig>(config: &T, storage: Storage) -> Result<Self> {
        let config = config.worker();

        let poll = Poll::new()?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        Ok(Self {
            nevent,
            poll,
            storage,
            timeout,
            waker,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    pub fn build(
        self,
        data_queue: Queues<(Request, Response, Token), (Request, Token)>,
        signal_queue: Queues<(), Signal>,
    ) -> StorageWorker<Request, Response, Storage> {
        StorageWorker {
            data_queue,
            nevent: self.nevent,
            poll: self.poll,
            signal_queue,
            storage: self.storage,
            timeout: self.timeout,
            waker: self.waker,
            _request: PhantomData,
            _response: PhantomData,
        }
    }
}

pub enum Workers<Parser, Request, Response, Storage> {
    Single {
        worker: SingleWorker<Parser, Request, Response, Storage>,
    },
    Multi {
        workers: Vec<MultiWorker<Parser, Request, Response>>,
        storage: StorageWorker<Request, Response, Storage>,
    },
}

impl<Parser, Request, Response, Storage> Workers<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Send,
    Response: 'static + Compose + Send,
    Storage: 'static + EntryStore + Execute<Request, Response> + Send,
{
    pub fn spawn(self) -> Vec<JoinHandle<()>> {
        match self {
            Self::Single { mut worker } => {
                vec![std::thread::Builder::new()
                    .name(format!("{}_worker", THREAD_PREFIX))
                    .spawn(move || worker.run())
                    .unwrap()]
            }
            Self::Multi {
                mut workers,
                mut storage,
            } => {
                let mut join_handles = vec![std::thread::Builder::new()
                    .name(format!("{}_storage", THREAD_PREFIX))
                    .spawn(move || storage.run())
                    .unwrap()];

                for (id, mut worker) in workers.drain(..).enumerate() {
                    join_handles.push(
                        std::thread::Builder::new()
                            .name(format!("{}_worker_{}", THREAD_PREFIX, id))
                            .spawn(move || worker.run())
                            .unwrap(),
                    )
                }

                join_handles
            }
        }
    }
}

pub enum WorkersBuilder<Parser, Request, Response, Storage> {
    Single {
        worker: SingleWorkerBuilder<Parser, Request, Response, Storage>,
    },
    Multi {
        workers: Vec<MultiWorkerBuilder<Parser, Request, Response>>,
        storage: StorageWorkerBuilder<Request, Response, Storage>,
    },
}

impl<Parser, Request, Response, Storage> WorkersBuilder<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Clone,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser, storage: Storage) -> Result<Self> {
        let threads = config.worker().threads();

        if threads > 1 {
            let mut workers = vec![];
            for _ in 0..threads {
                workers.push(MultiWorkerBuilder::new(config, parser.clone())?)
            }

            Ok(Self::Multi {
                workers,
                storage: StorageWorkerBuilder::new(config, storage)?,
            })
        } else {
            Ok(Self::Single {
                worker: SingleWorkerBuilder::new(config, parser, storage)?,
            })
        }
    }

    pub fn worker_wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker.clone()]
            }
            Self::Multi {
                workers,
                storage: _,
            } => workers.iter().map(|w| w.waker.clone()).collect(),
        }
    }

    pub fn wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker.clone()]
            }
            Self::Multi {
                workers,
                storage,
            } => {
            	let mut wakers = vec![storage.waker.clone()];
            	for worker in workers {
            		wakers.push(worker.waker.clone());
            	}
            	wakers
            },
        }
    }

    pub fn build(
        self,
        session_queues: Vec<Queues<Session, Session>>,
        signal_queues: Vec<Queues<(), Signal>>,
    ) -> Workers<Parser, Request, Response, Storage> {
        let mut signal_queues = signal_queues;
        let mut session_queues = session_queues;
        match self {
            Self::Multi {
                storage,
                mut workers,
            } => {
                let storage_wakers = vec![storage.waker.clone()];
                let worker_wakers: Vec<Arc<Waker>> =
                    workers.iter().map(|v| v.waker.clone()).collect();
                let (mut worker_data_queues, mut storage_data_queues) =
                    Queues::new(worker_wakers, storage_wakers, QUEUE_CAPACITY);

                // The storage thread precedes the worker threads in the set of
                // wakers, so its signal queue is the first element of
                // `signal_queues`. Its request queue is also the first (and
                // only) element of `request_queues`. We remove these and build
                // the storage so we can loop through the remaining signal
                // queues when launching the worker threads.
                let s = storage.build(storage_data_queues.remove(0), signal_queues.remove(0));

                let mut w = Vec::new();
                for worker_builder in workers.drain(..) {
                    w.push(worker_builder.build(
                        worker_data_queues.remove(0),
                        session_queues.remove(0),
                        signal_queues.remove(0),
                    ));
                }

                Workers::Multi {
                    storage: s,
                    workers: w,
                }
            }
            Self::Single { worker } => Workers::Single {
                worker: worker.build(session_queues.remove(0), signal_queues.remove(0)),
            },
        }
    }
}
