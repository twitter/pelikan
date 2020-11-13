use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use buffer::Buffer;
use clap::{App, Arg, ArgMatches};
use mio::net::TcpStream;
use mio::*;
use rustcommon_atomics::*;
use rustcommon_histogram::*;
use rustcommon_logger::*;
use rustls::{ClientConfig, ClientSession, Session as TlsSession};
use slab::Slab;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "pingclient";

fn main() {
    // initialize logging
    Logger::new()
        .label(NAME)
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    let app = App::new(NAME)
        .version(VERSION)
        .author("Brian Martin <bmartin@twitter.com>")
        .about("Simple pingserver client as a basic benchmark with TLS support")
        .arg(
            Arg::with_name("certificate-chain")
                .long("certificate-chain")
                .value_name("FILE")
                .help("Specify the certificate chain to validate server certificate")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("certificate")
                .long("certificate")
                .value_name("FILE")
                .help("Specify the client certificate")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("private-key")
                .long("private-key")
                .value_name("FILE")
                .help("Specify the client private key")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("duration")
                .long("duration")
                .value_name("SECONDS")
                .help("Runtime of the test in seconds")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("endpoint")
                .long("endpoint")
                .value_name("HOST:PORT")
                .help("Specify the endpoint HOST:PORT")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("clients")
                .long("clients")
                .value_name("THREADS")
                .help("Number of client threads")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("poolsize")
                .long("poolsize")
                .value_name("CONNECTIONS")
                .help("Number of connections to open to the endpoint")
                .required(false)
                .takes_value(true),
        );

    let matches = app.get_matches();

    let endpoint: SocketAddr = matches
        .value_of("endpoint")
        .unwrap()
        .parse()
        .unwrap_or_else(|_| {
            fatal!(
                "Invalid endpoint: {}",
                matches.value_of("endpoint").unwrap()
            );
        });

    let poolsize = matches
        .value_of("poolsize")
        .unwrap_or("1")
        .parse()
        .unwrap_or_else(|_| {
            fatal!(
                "Invalid poolsize: {}",
                matches.value_of("poolsize").unwrap()
            );
        });

    let clients: usize = matches
        .value_of("clients")
        .unwrap_or("1")
        .parse()
        .unwrap_or_else(|_| {
            fatal!("Invalid clients: {}", matches.value_of("clients").unwrap());
        });

    let duration = matches
        .value_of("duration")
        .unwrap_or("60")
        .parse()
        .unwrap_or_else(|_| {
            fatal!(
                "Invalid duration: {}",
                matches.value_of("duration").unwrap()
            );
        });

    let histogram = Arc::new(AtomicHistogram::<u64, AtomicU64>::new(1_000_000_000, 3));
    let responses = Arc::new(AtomicU64::new(0));

    info!("launching");

    let config = load_tls_config(&matches);

    let running = Arc::new(AtomicBool::new(true));

    let mut threads = Vec::new();

    for _thread in 0..clients {
        let endpoint = endpoint.clone();
        let poolsize = poolsize;
        let config = config.clone();
        let running = running.clone();
        let histogram = histogram.clone();
        let responses = responses.clone();
        let handle = std::thread::spawn(move || {
            worker(endpoint, poolsize, config, running, histogram, responses)
        });
        threads.push(handle);
    }

    let start = Instant::now();
    let mut now: Instant;
    loop {
        now = Instant::now();
        if start.elapsed() >= Duration::new(duration, 0) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    running.store(false, Ordering::Relaxed);

    for thread in threads {
        let _ = thread.join();
    }

    let elapsed = now - start;
    let rate = responses.load(Ordering::SeqCst) as f64
        / (elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0);
    info!("rate: {:.2} rps", rate);
    info!("min: {} ns", histogram.percentile(0.00).unwrap_or(0));
    info!("p01: {} ns", histogram.percentile(0.01).unwrap_or(0));
    info!("p10: {} ns", histogram.percentile(0.10).unwrap_or(0));
    info!("p25: {} ns", histogram.percentile(0.25).unwrap_or(0));
    info!("p50: {} ns", histogram.percentile(0.50).unwrap_or(0));
    info!("p75: {} ns", histogram.percentile(0.75).unwrap_or(0));
    info!("p90: {} ns", histogram.percentile(0.90).unwrap_or(0));
    info!("p99: {} ns", histogram.percentile(0.99).unwrap_or(0));
    info!("max: {} ns", histogram.percentile(1.00).unwrap_or(0));
}

fn worker(
    endpoint: SocketAddr,
    poolsize: usize,
    config: Option<Arc<ClientConfig>>,
    running: Arc<AtomicBool>,
    histogram: Arc<AtomicHistogram<u64, AtomicU64>>,
    responses: Arc<AtomicU64>,
) {
    let mut poll = Poll::new()
        .map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })
        .unwrap();

    let mut sessions = Slab::new();

    for _ in 0..poolsize {
        let s = sessions.vacant_entry();
        let mut session = Session::new(
            endpoint,
            config.as_ref(),
            s.key(),
            histogram.clone(),
            responses.clone(),
        );
        session.register(&poll);
        s.insert(session);
    }

    let mut events = Events::with_capacity(1024);

    while running.load(Ordering::Relaxed) {
        let _ = poll.poll(&mut events, Some(std::time::Duration::from_millis(100)));
        for event in &events {
            if let Some(session) = sessions.get_mut(event.token().0) {
                if event.is_readable() {
                    if session.do_read().is_err() {
                        error!("read error");
                        let mut s = sessions.remove(event.token().0);
                        s.deregister(&poll);
                        continue;
                    } else {
                        trace!("read ok");
                    }
                }

                if event.is_writable() {
                    if session.do_write().is_err() {
                        error!("write error");
                    } else {
                        trace!("write ok");
                    }
                }

                session.reregister(&poll);
            }
        }
    }
}

fn load_tls_config(matches: &ArgMatches) -> Option<Arc<rustls::ClientConfig>> {
    let cert_chain = matches.value_of("certificate-chain");
    let cert = matches.value_of("certificate");
    let key = matches.value_of("private-key");

    if cert_chain.is_some() && cert.is_some() && key.is_some() {
        let mut config = rustls::ClientConfig::new();

        let certificate_chain =
            std::fs::File::open(cert_chain.unwrap()).expect("failed to open cert chain");
        config
            .root_store
            .add_pem_file(&mut std::io::BufReader::new(certificate_chain))
            .expect("failed to load cert chain");

        config
            .dangerous()
            .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

        let cert = std::fs::File::open(cert.unwrap()).expect("failed to open cert");
        let cert = rustls::internal::pemfile::certs(&mut std::io::BufReader::new(cert)).unwrap();

        let key = std::fs::File::open(key.unwrap()).expect("failed to open private key");
        let keys = rustls::internal::pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(key))
            .unwrap();
        assert_eq!(keys.len(), 1);
        let key = keys[0].clone();

        config
            .set_single_client_cert(cert, key)
            .expect("invalid cert or key");

        Some(Arc::new(config))
    } else if cert_chain.is_none() && cert.is_none() && key.is_none() {
        None
    } else {
        fatal!("Invalid TLS configuration");
    }
}

enum State {
    Connecting,
    Reading,
    Writing,
}

struct Session {
    stream: TcpStream,
    state: State,
    tls: Option<ClientSession>,
    buffer: Buffer,
    token: Token,
    t0: Instant,
    histogram: Arc<AtomicHistogram<u64, AtomicU64>>,
    responses: Arc<AtomicU64>,
}

impl Session {
    pub fn new(
        addr: SocketAddr,
        tls_config: Option<&Arc<ClientConfig>>,
        token: usize,
        histogram: Arc<AtomicHistogram<u64, AtomicU64>>,
        responses: Arc<AtomicU64>,
    ) -> Self {
        let stream = TcpStream::connect(addr).expect("failed to open connection");
        let tls = if let Some(ref tls_config) = tls_config {
            let mut tls = rustls::ClientSession::new(
                &tls_config,
                webpki::DNSNameRef::try_from_ascii_str("localhost").expect("invalid dns name"),
            );
            let _ = tls.write(b"PING\r\n");
            Some(tls)
        } else {
            None
        };
        let state = if tls_config.is_some() {
            State::Connecting
        } else {
            State::Writing
        };
        let buffer = Buffer::new(4096, 4096);
        Self {
            stream,
            state,
            tls,
            buffer,
            token: Token(token),
            t0: Instant::now(),
            histogram,
            responses,
        }
    }

    pub fn interests(&self) -> Interest {
        if let Some(ref tls) = self.tls {
            let r = tls.wants_read();
            let w = tls.wants_write();

            if r && w {
                mio::Interest::READABLE | mio::Interest::WRITABLE
            } else if w {
                mio::Interest::WRITABLE
            } else {
                mio::Interest::READABLE
            }
        } else {
            match &self.state {
                State::Reading => Interest::READABLE,
                State::Writing => Interest::WRITABLE,
                State::Connecting => Interest::READABLE | Interest::WRITABLE,
            }
        }
    }

    pub fn register(&mut self, poll: &Poll) {
        let interests = self.interests();
        poll.registry()
            .register(&mut self.stream, self.token, interests)
            .unwrap();
    }

    pub fn reregister(&mut self, poll: &Poll) {
        let interests = self.interests();
        poll.registry()
            .reregister(&mut self.stream, self.token, interests)
            .unwrap();
    }

    pub fn deregister(&mut self, poll: &Poll) {
        poll.registry().deregister(&mut self.stream).unwrap();
    }

    pub fn do_read(&mut self) -> Result<(), ()> {
        trace!("do read");
        if let Some(ref mut tls) = self.tls {
            match tls.read_tls(&mut self.stream) {
                Err(_) => Err(()),
                Ok(0) => Err(()),
                Ok(_) => {
                    if tls.process_new_packets().is_err() {
                        Err(())
                    } else {
                        let _ = self.buffer.read_from(tls);
                        if self.buffer.rx_buffer() == b"PONG\r\n" {
                            let elapsed = self.t0.elapsed();
                            let nanos = elapsed.as_secs() as u64 * 1_000_000_000
                                + elapsed.subsec_nanos() as u64;
                            self.histogram.increment(nanos, 1);
                            self.responses.fetch_add(1, Ordering::Relaxed);
                            self.buffer.clear();
                            self.state = State::Writing;
                            self.t0 = Instant::now();
                            let _ = tls.write(b"PING\r\n");
                        }
                        Ok(())
                    }
                }
            }
        } else {
            let _ = self.buffer.read_from(&mut self.stream);
            if self.buffer.rx_buffer() == b"PONG\r\n" {
                let elapsed = self.t0.elapsed();
                let nanos =
                    elapsed.as_secs() as u64 * 1_000_000_000 + elapsed.subsec_nanos() as u64;
                self.histogram.increment(nanos, 1);
                self.responses.fetch_add(1, Ordering::Relaxed);
                self.buffer.clear();
                self.state = State::Writing;
            }
            Ok(())
        }
    }

    pub fn do_write(&mut self) -> Result<(), ()> {
        trace!("do write");
        if let Some(ref mut tls) = self.tls {
            if tls.write_tls(&mut self.stream).is_err() {
                Err(())
            } else {
                match self.buffer.write_to(tls) {
                    Ok(Some(6)) => {
                        // tx complete
                        Ok(())
                    }
                    Ok(_) => {
                        // incomplete
                        Ok(())
                    }
                    Err(_) => Err(()),
                }
            }
        } else {
            self.state = State::Reading;
            self.t0 = Instant::now();
            let _ = self.stream.write(b"PING\r\n");
            Ok(())
        }
    }
}

pub struct NoCertificateVerification {}

impl rustls::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _roots: &rustls::RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: webpki::DNSNameRef<'_>,
        _ocsp: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        Ok(rustls::ServerCertVerified::assertion())
    }
}
