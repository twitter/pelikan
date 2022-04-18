#[macro_use]
extern crate logger;

use backtrace::Backtrace;
use clap::{App, Arg};
use config::*;
use core::num::NonZeroU64;
use core::num::NonZeroUsize;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;
use logger::configure_logging;
use logger::Drain;
use metrics::*;
use momento::response::cache_get_response::*;
use momento::response::cache_set_response::*;
use momento::response::error::*;
use momento::simple_cache_client::*;
use protocol_admin::*;
use protocol_memcache::*;
use session::*;
use std::borrow::{Borrow, BorrowMut};
use std::io::{Error, ErrorKind};
use storage_types::OwnedValue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::time::timeout;

pub const KB: usize = 1024;
pub const MB: usize = 1024 * KB;

const S: u64 = 1_000_000_000; // one second in nanoseconds
const US: u64 = 1_000; // one microsecond in nanoseconds

mod admin;
mod commands;
mod frontend;
mod listener;

// NOTES:
//
// This is a simple proxy which translates requests between memcache protocol
// and Momento gRPC. This allows for a standard memcache client to communicate
// with the Momento cache service without any code changes.
//
// The following environment variables are necessary to configure the proxy
// until the config file is finalized:
//
// MOMENTO_AUTHENTICATION - the Momento authentication token

// the default buffer size is matched to the upper-bound on TLS fragment size as
// per RFC 5246 https://datatracker.ietf.org/doc/html/rfc5246#section-6.2.1
pub const INITIAL_BUFFER_SIZE: usize = 16 * KB;

// sets an upper bound on how large a request can be
pub const MAX_REQUEST_SIZE: usize = 100 * MB;

// The Momento cache client requires providing a default TTL. For the current
// implementation of the proxy, we don't actually let the client use the default,
// we always specify a TTL for each `set`.
const DEFAULT_TTL_SECONDS: NonZeroU64 = unsafe { NonZeroU64::new_unchecked(3600) };

// we interpret TTLs the same way memcached would
pub const TIME_TYPE: TimeType = TimeType::Memcache;

pub static PERCENTILES: &[(&str, f64)] = &[
    ("p25", 25.0),
    ("p50", 50.0),
    ("p75", 75.0),
    ("p90", 90.0),
    ("p99", 99.0),
    ("p999", 99.9),
    ("p9999", 99.99),
];

// define metrics that are part of the proxy
static_metrics! {
    static ADMIN_REQUEST_PARSE: Counter;
    static ADMIN_RESPONSE_COMPOSE: Counter;

    static BACKEND_REQUEST: Counter;
    static BACKEND_EX: Counter;
    static BACKEND_EX_RATE_LIMITED: Counter;
    static BACKEND_EX_TIMEOUT: Counter;

    static GET: Counter;
    static GET_EX: Counter;
    static GET_KEY: Counter;
    static GET_KEY_EX: Counter;
    static GET_KEY_HIT: Counter;
    static GET_KEY_MISS: Counter;

    static SET: Counter;
    static SET_EX: Counter;
    static SET_NOT_STORED: Counter;
    static SET_STORED: Counter;

    static RU_UTIME: Counter;
    static RU_STIME: Counter;
    static RU_MAXRSS: Gauge;
    static RU_IXRSS: Gauge;
    static RU_IDRSS: Gauge;
    static RU_ISRSS: Gauge;
    static RU_MINFLT: Counter;
    static RU_MAJFLT: Counter;
    static RU_NSWAP: Counter;
    static RU_INBLOCK: Counter;
    static RU_OUBLOCK: Counter;
    static RU_MSGSND: Counter;
    static RU_MSGRCV: Counter;
    static RU_NSIGNALS: Counter;
    static RU_NVCSW: Counter;
    static RU_NIVCSW: Counter;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        println!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // parse command line options
    let matches = App::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .version_short("v")
        .long_about(
            "A proxy that supports a limited subset of the Memcache protocol on
            the client side and communicates with Momento over gRPC to fulfill
            the requests.

            This allows use of the Momento cache offering without code changes
            for existing software which uses Memcached.

            The supported commands are limited to: get/set",
        )
        // .arg(
        //     Arg::with_name("stats")
        //         .short("s")
        //         .long("stats")
        //         .help("List all metrics in stats")
        //         .takes_value(false),
        // )
        .arg(
            Arg::with_name("CONFIG")
                .help("Server configuration file")
                .index(1),
        )
        .get_matches();

    // load config from file
    let config = if let Some(file) = matches.value_of("CONFIG") {
        match MomentoProxyConfig::load(file) {
            Ok(c) => c,
            Err(e) => {
                println!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        Default::default()
    };

    // initialize logging
    let mut log_drain = configure_logging(&config);

    // validate config parameters
    for cache in config.caches() {
        let name = cache.cache_name();
        let ttl = cache.default_ttl();
        let limit = u64::MAX / 1000;
        if ttl.get() > limit {
            error!("default ttl of {ttl} for cache `{name}` is greater than {limit}");
            let _ = log_drain.flush();
            std::process::exit(1);
        }

        if let Err(e) = cache.socket_addr() {
            error!("listen address for cache `{name}` is not valid: {}", e);
            let _ = log_drain.flush();
            std::process::exit(1);
        }
    }

    // initialize metrics
    metrics::init();

    let mut runtime = Builder::new_multi_thread();

    runtime.thread_name_fn(|| {
        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
        let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
        format!("pelikan_wrk_{}", id)
    });

    if let Some(threads) = config.threads() {
        runtime.worker_threads(threads);
    }

    let runtime = runtime
        .enable_all()
        .build()
        .expect("failed to launch tokio runtime");

    runtime.block_on(async move { spawn(config, log_drain).await })
}

async fn spawn(
    config: MomentoProxyConfig,
    mut log_drain: Box<dyn Drain>,
) -> Result<(), Box<dyn std::error::Error>> {
    let admin_addr = config
        .admin()
        .socket_addr()
        .expect("bad admin listen address");
    let admin_listener = TcpListener::bind(&admin_addr).await?;
    info!("starting proxy admin listener on: {}", admin_addr);

    // initialize the Momento cache client
    if std::env::var("MOMENTO_AUTHENTICATION").is_err() {
        error!("environment variable `MOMENTO_AUTHENTICATION` is not set");
        let _ = log_drain.flush();
        std::process::exit(1);
    }
    let auth_token =
        std::env::var("MOMENTO_AUTHENTICATION").expect("MOMENTO_AUTHENTICATION must be set");
    let client_builder = match SimpleCacheClientBuilder::new(auth_token, DEFAULT_TTL_SECONDS).await
    {
        Ok(c) => c,
        Err(e) => {
            error!("could not create cache client: {}", e);
            let _ = log_drain.flush();
            std::process::exit(1);
        }
    };

    if config.caches().is_empty() {
        error!("no caches specified in the config");
        let _ = log_drain.flush();
        std::process::exit(1);
    }

    for i in 0..config.caches().len() {
        let config = config.clone();
        let client_builder = client_builder.clone();

        let cache = config.caches().get(i).unwrap().clone();
        let addr = match cache.socket_addr() {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "bad listen address for cache `{}`: {}",
                    cache.cache_name(),
                    e
                );
                let _ = log_drain.flush();
                std::process::exit(1);
            }
        };
        let ttl = cache.default_ttl();

        let tcp_listener = match std::net::TcpListener::bind(&addr) {
            Ok(v) => {
                if let Err(e) = v.set_nonblocking(true) {
                    error!(
                        "could not set tcp listener for cache `{}` on address `{}` as non-blocking: {}",
                        cache.cache_name(),
                        addr,
                        e
                    );
                    let _ = log_drain.flush();
                    std::process::exit(1);
                }
                v
            }
            Err(e) => {
                error!(
                    "could not bind tcp listener for cache `{}` on address `{}`: {}",
                    cache.cache_name(),
                    addr,
                    e
                );
                let _ = log_drain.flush();
                std::process::exit(1);
            }
        };

        tokio::spawn(async move {
            let client_builder = client_builder
                .default_ttl_seconds(ttl)
                .expect("bad default ttl");

            info!(
                "starting proxy frontend listener for cache `{}` on: {}",
                cache.cache_name(),
                addr
            );
            let tcp_listener =
                TcpListener::from_std(tcp_listener).expect("could not convert to tokio listener");
            listener::listener(tcp_listener, client_builder, cache.cache_name()).await;
        });
    }

    admin::admin(log_drain, admin_listener).await;
    Ok(())
}

async fn do_read(
    socket: &mut tokio::net::TcpStream,
    buf: &mut Buffer,
) -> Result<NonZeroUsize, Error> {
    match socket.read(buf.borrow_mut()).await {
        Ok(0) => {
            SESSION_RECV.increment();
            // zero length reads mean we got a HUP. close it
            Err(Error::from(ErrorKind::ConnectionReset))
        }
        Ok(n) => {
            SESSION_RECV.increment();
            SESSION_RECV_BYTE.add(n as _);
            TCP_RECV_BYTE.add(n as _);
            // non-zero means we have some data, mark the buffer as
            // having additional content
            buf.increase_len(n);

            // if the buffer is low on space, we will grow the
            // buffer
            if buf.available_capacity() * 2 < INITIAL_BUFFER_SIZE {
                buf.reserve(INITIAL_BUFFER_SIZE);
            }

            // SAFETY: we have already checked that the number of bytes read was
            // greater than zero, so this unchecked conversion is safe
            Ok(unsafe { NonZeroUsize::new_unchecked(n) })
        }
        Err(e) => {
            SESSION_RECV.increment();
            SESSION_RECV_EX.increment();
            // we has some other error reading from the socket,
            // just close it
            // break;
            Err(e)
        }
    }
}
