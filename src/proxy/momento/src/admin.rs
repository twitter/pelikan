// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use buffer::Buf;

pub(crate) async fn admin(mut log_drain: Box<dyn logger::Drain>, admin_listener: TcpListener) {
    loop {
        let _ = log_drain.flush();

        // accept a new client
        if let Ok(Ok((socket, _))) =
            timeout(Duration::from_millis(1), admin_listener.accept()).await
        {
            // TCP_CONN_CURR.increment();
            // TCP_ACCEPT.increment();
            tokio::spawn(async move {
                admin::handle_admin_client(socket).await;
                // TCP_CLOSE.increment();
                // TCP_CONN_CURR.decrement();
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

async fn handle_admin_client(mut socket: tokio::net::TcpStream) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::new(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = AdminRequestParser::new();
    loop {
        if do_read(&mut socket, &mut buf).await.is_err() {
            break;
        }

        // ADMIN_REQUEST_PARSE.increment();

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
                buf.advance(consumed);
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {}
                _ => {
                    // invalid request
                    let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                    break;
                }
            },
        }
    }
}

async fn stats_response(socket: &mut tokio::net::TcpStream) -> Result<(), Error> {
    let mut data = Vec::new();
    for metric in &rustcommon_metrics::metrics() {
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
    // ADMIN_RESPONSE_COMPOSE.increment();
    for line in data {
        socket.write_all(line.as_bytes()).await?;
    }
    socket.write_all(b"END\r\n").await
}
