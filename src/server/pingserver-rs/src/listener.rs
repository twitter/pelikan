use crate::session::*;
use crate::*;

/// A `Listener` is used to bind to a given socket address and accept new
/// sessions. These sessions are moved onto a MPSC queue, where they can be
/// handled by a `Worker`.
pub struct Listener {
    addr: SocketAddr,
    config: Arc<PingserverConfig>,
    listener: TcpListener,
    poll: Poll,
    sender: Sender<Session>,
}

impl Listener {
    /// Creates a new `Listener` that will bind to a given `addr` and push new
    /// `Session`s over the `sender`
    pub fn new(
        config: Arc<PingserverConfig>,
        sender: Sender<Session>,
    ) -> Result<Self, std::io::Error> {
        let addr = config.server().socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;
        let listener = TcpListener::bind(&addr).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to start tcp listener")
        })?;
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        // register listener to event loop
        poll.register(&listener, Token(0), Ready::readable(), PollOpt::edge())
            .map_err(|e| {
                error!("{}", e);
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to register listener with epoll",
                )
            })?;

        Ok(Self {
            addr,
            config,
            listener,
            poll,
            sender,
        })
    }

    /// Runs the `Listener` in a loop, accepting new sessions and moving them to
    /// the queue
    pub fn run(&mut self) {
        info!("running listener on: {}", self.addr);

        let mut events = Events::with_capacity(self.config.server().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.server().timeout() as u64,
        ));

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling listener");
            }
            for event in events.iter() {
                if event.token() == Token(0) {
                    if let Ok((stream, addr)) = self.listener.accept() {
                        let client = Session::new(addr, stream, State::Reading);
                        if self.sender.send(client).is_err() {
                            println!("error sending client to worker");
                        }
                    } else {
                        println!("error accepting client");
                    }
                } else {
                    println!("unknown token");
                }
            }
        }
    }
}
