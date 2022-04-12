#[macro_use]
extern crate logger;

use core::num::NonZeroU64;
use backtrace::Backtrace;
use clap::{App, Arg};
use config::*;
use core::num::NonZeroUsize;
use core::time::Duration;
use logger::configure_logging;
use metrics::*;
use momento::response::cache_get_response::*;
use momento::response::cache_set_response::*;
use momento::response::error::*;
use momento::simple_cache_client::*;
use protocol::admin::*;
use protocol::memcache::*;
use protocol::{Parse, ParseError};
use session::*;
use std::borrow::{Borrow, BorrowMut};
use std::io::{Error, ErrorKind};
use storage_types::OwnedValue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

pub const KB: usize = 1024;
pub const MB: usize = 1024 * KB;

const S: u64 = 1_000_000_000; // one second in nanoseconds
const US: u64 = 1_000; // one microsecond in nanoseconds

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

    static GET_KEY_EX: Counter;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            listener(tcp_listener, client_builder, cache.cache_name()).await;
        });
    }

    admin(log_drain, admin_listener).await;
    Ok(())
}

async fn listener(
    listener: TcpListener,
    client_builder: SimpleCacheClientBuilder,
    cache_name: String,
) {
    // this acts as our listener thread and spawns tasks for each client
    loop {
        // accept a new client
        if let Ok((socket, _)) = listener.accept().await {
            TCP_ACCEPT.increment();

            let client = client_builder.clone().build();
            let cache_name = cache_name.clone();

            // spawn a task for managing requests for the client
            tokio::spawn(async move {
                TCP_CONN_CURR.increment();
                handle_proxy_client(socket, client, cache_name).await;

                TCP_CLOSE.increment();
                TCP_CONN_CURR.decrement();
            });
        }
    }
}

async fn admin(mut log_drain: Box<dyn logger::Drain>, admin_listener: TcpListener) {
    loop {
        let _ = log_drain.flush();

        // accept a new client
        if let Ok(Ok((socket, _))) =
            timeout(Duration::from_millis(1), admin_listener.accept()).await
        {
            TCP_CONN_CURR.increment();
            TCP_ACCEPT.increment();
            tokio::spawn(async move {
                handle_admin_client(socket).await;
                TCP_CLOSE.increment();
                TCP_CONN_CURR.decrement();
            });
        };

        let mut rusage = libc::rusage {
            ru_utime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_stime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_maxrss: 0,
            ru_ixrss: 0,
            ru_idrss: 0,
            ru_isrss: 0,
            ru_minflt: 0,
            ru_majflt: 0,
            ru_nswap: 0,
            ru_inblock: 0,
            ru_oublock: 0,
            ru_msgsnd: 0,
            ru_msgrcv: 0,
            ru_nsignals: 0,
            ru_nvcsw: 0,
            ru_nivcsw: 0,
        };

        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage) } == 0 {
            RU_UTIME.set(rusage.ru_utime.tv_sec as u64 * S + rusage.ru_utime.tv_usec as u64 * US);
            RU_STIME.set(rusage.ru_stime.tv_sec as u64 * S + rusage.ru_stime.tv_usec as u64 * US);
            RU_MAXRSS.set(rusage.ru_maxrss * KB as i64);
            RU_IXRSS.set(rusage.ru_ixrss * KB as i64);
            RU_IDRSS.set(rusage.ru_idrss * KB as i64);
            RU_ISRSS.set(rusage.ru_isrss * KB as i64);
            RU_MINFLT.set(rusage.ru_minflt as u64);
            RU_MAJFLT.set(rusage.ru_majflt as u64);
            RU_NSWAP.set(rusage.ru_nswap as u64);
            RU_INBLOCK.set(rusage.ru_inblock as u64);
            RU_OUBLOCK.set(rusage.ru_oublock as u64);
            RU_MSGSND.set(rusage.ru_msgsnd as u64);
            RU_MSGRCV.set(rusage.ru_msgrcv as u64);
            RU_NSIGNALS.set(rusage.ru_nsignals as u64);
            RU_NVCSW.set(rusage.ru_nvcsw as u64);
            RU_NIVCSW.set(rusage.ru_nivcsw as u64);
        }

        tokio::time::sleep(core::time::Duration::from_millis(100)).await;
    }
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

async fn stats_response(socket: &mut tokio::net::TcpStream) -> Result<(), Error> {
    let mut data = Vec::new();
    for metric in &metrics::common::metrics::metrics() {
        let any = match metric.as_any() {
            Some(any) => any,
            None => {
                continue;
            }
        };

        // we need to filter some irrelvant metrics that
        // are defined in crates we depend on
        if metric.name().starts_with("add")
            || metric.name().starts_with("append")
            || metric.name().starts_with("cas")
            || metric.name().starts_with("decr")
            || metric.name().starts_with("delete")
            || metric.name().starts_with("gets")
            || metric.name().starts_with("incr")
            || metric.name().starts_with("get_cardinality")
            || metric.name().starts_with("ping")
            || metric.name().starts_with("pipeline_depth")
            || metric.name().starts_with("prepend")
            || metric.name().starts_with("replace")
            || metric.name().starts_with("request_latency")
        {
            continue;
        }

        if let Some(counter) = any.downcast_ref::<Counter>() {
            data.push(format!("STAT {} {}\r\n", metric.name(), counter.value()));
        } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
            data.push(format!("STAT {} {}\r\n", metric.name(), gauge.value()));
        } else if let Some(heatmap) = any.downcast_ref::<Heatmap>() {
            for (label, value) in PERCENTILES {
                let percentile = heatmap.percentile(*value).unwrap_or(0);
                data.push(format!(
                    "STAT {}_{} {}\r\n",
                    metric.name(),
                    label,
                    percentile
                ));
            }
        }
    }

    data.sort();
    ADMIN_RESPONSE_COMPOSE.increment();
    for line in data {
        socket.write_all(line.as_bytes()).await?;
    }
    socket.write_all(b"END\r\n").await
}

async fn handle_admin_client(mut socket: tokio::net::TcpStream) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::with_capacity(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = AdminRequestParser::new();
    loop {
        if do_read(&mut socket, &mut buf).await.is_err() {
            break;
        }

        ADMIN_REQUEST_PARSE.increment();

        match parser.parse(buf.borrow()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();

                match request {
                    AdminRequest::Stats { .. } => {
                        if stats_response(&mut socket).await.is_err() {
                            break;
                        }
                    }
                    _ => {
                        debug!("unsupported command: {:?}", request);
                    }
                }
                buf.consume(consumed);
            }
            Err(ParseError::Incomplete) => {}
            Err(ParseError::Invalid) => {
                // invalid request
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
            Err(ParseError::UnknownCommand) => {
                // unknown command
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
        }
    }
}

async fn get(
    client: &mut SimpleCacheClient,
    cache_name: &str,
    socket: &mut tokio::net::TcpStream,
    keys: &[Box<[u8]>],
) -> Result<(), Error> {
    GET.increment();

    // check if any of the keys are invalid before
    // sending the requests to the backend
    for key in keys.iter() {
        if std::str::from_utf8(key).is_err() {
            GET_EX.increment();

            // invalid key
            let _ = socket.write_all(b"ERROR\r\n").await;
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }

    let mut response_buf = Vec::new();

    for key in keys.iter() {
        BACKEND_REQUEST.increment();
        GET_KEY.increment();

        // we've already checked the keys, so we
        // know this unwrap is safe
        let key = std::str::from_utf8(key).unwrap();

        match timeout(Duration::from_millis(200), client.get(&cache_name, key)).await {
            Ok(Ok(response)) => {
                match response.result {
                    MomentoGetStatus::ERROR => {
                        // we got some error from
                        // the backend.
                        BACKEND_EX.increment();

                        GET_KEY_EX.increment();

                        // TODO: what is the right
                        // way to handle this?
                        //
                        // currently ignoring and
                        // moving on to the next key
                    }
                    MomentoGetStatus::HIT => {
                        GET_KEY_HIT.increment();

                        let length = response.value.len();

                        let item_header = format!("VALUE {} 0 {}\r\n", key, length);

                        let response_len = 2 + item_header.len() + response.value.len();

                        klog_get(key, response_len);

                        response_buf.extend_from_slice(item_header.as_bytes());
                        response_buf.extend_from_slice(&response.value);
                        response_buf.extend_from_slice(b"\r\n");
                    }
                    MomentoGetStatus::MISS => {
                        GET_KEY_MISS.increment();

                        // we don't write anything for a miss

                        klog_get(key, 0);
                    }
                }
            }
            Ok(Err(MomentoError::LimitExceeded(_))) => {
                BACKEND_EX.increment();
                BACKEND_EX_RATE_LIMITED.increment();
                GET_KEY_EX.increment();
            }
            Ok(Err(e)) => {
                // we got some error from the momento client
                // log and incr stats and move on treating it
                // as a miss
                error!("error for get: {}", e);
                GET_KEY_EX.increment();
                BACKEND_EX.increment();
            }
            Err(_) => {
                // we had a timeout, incr stats and move on
                // treating it as a miss
                GET_KEY_EX.increment();
                BACKEND_EX.increment();
                BACKEND_EX_TIMEOUT.increment();
            }
        }
    }
    response_buf.extend_from_slice(b"END\r\n");

    SESSION_SEND.increment();
    SESSION_SEND_BYTE.add(response_buf.len() as _);
    TCP_SEND_BYTE.add(response_buf.len() as _);
    if let Err(e) = socket.write_all(&response_buf).await {
        SESSION_SEND_EX.increment();
        return Err(e);
    }
    Ok(())
}

async fn set(
    client: &mut SimpleCacheClient,
    cache_name: &str,
    socket: &mut tokio::net::TcpStream,
    entry: MemcacheEntry,
    noreply: bool,
) -> Result<(), Error> {
    SET.increment();

    if let Ok(key) = std::str::from_utf8(&entry.key) {
        let value = match entry.value {
            Some(OwnedValue::Bytes(v)) => {
                if let Ok(value) = std::str::from_utf8(&v) {
                    value.to_owned()
                } else {
                    debug!("value is not valid utf8: {:?}", v);
                    let _ = socket.write_all(b"ERROR\r\n").await;
                    return Err(Error::from(ErrorKind::InvalidInput));
                }
            }
            Some(OwnedValue::U64(v)) => {
                format!("{}", v)
            }
            None => {
                error!("empty values are not supported by momento");
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(7);
                TCP_SEND_BYTE.add(7);
                if socket.write_all(b"ERROR\r\n").await.is_err() {
                    SESSION_SEND_EX.increment();
                }
                return Err(Error::from(ErrorKind::InvalidInput));
            }
        };

        BACKEND_REQUEST.increment();

        let ttl = if let Some(ttl) = entry.ttl {
            NonZeroU64::new(ttl.as_secs() as u64)
        } else {
            None
        };

        match timeout(
            Duration::from_millis(200),
            client.set(
                &cache_name,
                key,
                &value,
                ttl,
            ),
        )
        .await
        {
            Ok(Ok(result)) => {
                match result.result {
                    MomentoSetStatus::OK => {
                        SET_STORED.increment();
                        if noreply {
                            klog_set(
                                key,
                                entry.flags,
                                entry.ttl.map(|v| v.as_secs() as u32).unwrap_or(0),
                                value.len(),
                                5,
                                0,
                            );
                        } else {
                            klog_set(
                                key,
                                entry.flags,
                                entry.ttl.map(|v| v.as_secs() as u32).unwrap_or(0),
                                value.len(),
                                5,
                                8,
                            );
                            SESSION_SEND.increment();
                            SESSION_SEND_BYTE.add(8);
                            TCP_SEND_BYTE.add(8);
                            if let Err(e) = socket.write_all(b"STORED\r\n").await {
                                SESSION_SEND_EX.increment();
                                // hangup if we can't send a response back
                                return Err(e);
                            }
                        }
                    }
                    MomentoSetStatus::ERROR => {
                        SET_NOT_STORED.increment();
                        if noreply {
                            klog_set(
                                key,
                                entry.flags,
                                entry.ttl.map(|v| v.as_secs() as u32).unwrap_or(0),
                                value.len(),
                                9,
                                0,
                            );
                        } else {
                            klog_set(
                                key,
                                entry.flags,
                                entry.ttl.map(|v| v.as_secs() as u32).unwrap_or(0),
                                value.len(),
                                9,
                                12,
                            );
                            SESSION_SEND.increment();
                            SESSION_SEND_BYTE.add(12);
                            TCP_SEND_BYTE.add(12);
                            // let client know this wasn't stored
                            if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                                SESSION_SEND_EX.increment();
                                // hangup if we can't send a response back
                                return Err(e);
                            }
                        }
                    }
                }
            }
            Ok(Err(MomentoError::LimitExceeded(_))) => {
                BACKEND_EX.increment();
                BACKEND_EX_RATE_LIMITED.increment();

                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
            Ok(Err(e)) => {
                error!("error for set: {}", e);

                BACKEND_EX.increment();
                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
            Err(_) => {
                // timeout
                BACKEND_EX.increment();
                BACKEND_EX_TIMEOUT.increment();
                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
        }
    } else {
        SET_EX.increment();
        // invalid key
        SESSION_SEND.increment();
        SESSION_SEND_BYTE.add(7);
        TCP_SEND_BYTE.add(7);
        if socket.write_all(b"ERROR\r\n").await.is_err() {
            SESSION_SEND_EX.increment();
        }
        return Err(Error::from(ErrorKind::InvalidInput));
    }
    Ok(())
}

async fn handle_proxy_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::with_capacity(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = MemcacheRequestParser::new(MAX_REQUEST_SIZE, TIME_TYPE);

    // handle incoming data from the client
    loop {
        if do_read(&mut socket, &mut buf).await.is_err() {
            break;
        }

        match parser.parse(buf.borrow()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();

                match request {
                    MemcacheRequest::Get { keys, .. } => {
                        if get(&mut client, &cache_name, &mut socket, &keys)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    MemcacheRequest::Set { entry, noreply } => {
                        if set(&mut client, &cache_name, &mut socket, entry, noreply)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ => {
                        debug!("unsupported command: {}", request.command());
                    }
                }
                buf.consume(consumed);
            }
            Err(ParseError::Incomplete) => {}
            Err(ParseError::Invalid) => {
                // invalid request
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
            Err(ParseError::UnknownCommand) => {
                // unknown command
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
        }
    }
}

/// Logs GET or GETS
fn klog_get(key: &str, response_len: usize) {
    if response_len == 0 {
        klog!("\"get {}\" 0 {}", key, response_len);
    } else {
        klog!("\"get {}\" 4 {}", key, response_len);
    }
}

/// Logs SET, ADD, or REPLACE
fn klog_set(
    key: &str,
    flags: u32,
    ttl: u32,
    value_len: usize,
    result_code: usize,
    response_len: usize,
) {
    klog!(
        "\"set {} {} {} {}\" {} {}",
        key,
        flags,
        ttl,
        value_len,
        result_code,
        response_len
    );
}
