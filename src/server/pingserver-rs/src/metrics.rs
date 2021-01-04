// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_metrics::*;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter};

use std::sync::Arc;
use std::time::Instant;

/// Defines various statistics
#[derive(Debug, AsRefStr, EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum Stat {
    AdminEventError,
    AdminEventLoop,
    AdminEventRead,
    AdminEventTotal,
    AdminEventWrite,
    AdminRequestParse,
    AdminRequestParseEx,
    AdminResponseCompose,
    AdminResponseComposeEx,
    Pid,
    RequestParse,
    RequestParseEx,
    ResponseCompose,
    ResponseComposeEx,
    ServerEventError,
    ServerEventLoop,
    ServerEventRead,
    ServerEventTotal,
    ServerEventWrite,
    SessionRecv,
    SessionRecvByte,
    SessionRecvEx,
    SessionSend,
    SessionSendByte,
    SessionSendEx,
    TcpAccept,
    TcpAcceptEx,
    TcpClose,
    TcpConnect,
    TcpConnectEx,
    TcpRecv,
    TcpRecvByte,
    TcpRecvEx,
    TcpReject,
    TcpRejectEx,
    TcpSend,
    TcpSendByte,
    TcpSendEx,
    WorkerEventError,
    WorkerEventLoop,
    WorkerEventRead,
    WorkerEventTotal,
    WorkerEventWake,
    WorkerEventWrite,
}

impl Statistic<AtomicU64, AtomicU64> for Stat {
    fn name(&self) -> &str {
        self.as_ref()
    }

    fn source(&self) -> Source {
        match self {
            &Stat::Pid => Source::Gauge,
            _ => Source::Counter,
        }
    }

    fn summary(&self) -> Option<Summary<AtomicU64, AtomicU64>> {
        None
    }
}

pub fn init() -> Arc<Metrics<AtomicU64, AtomicU64>> {
    let metrics = Arc::new(Metrics::<AtomicU64, AtomicU64>::new());

    metrics.add_output(&Stat::Pid, Output::Reading);
    let _ = metrics.record_gauge(&Stat::Pid, Instant::now(), std::process::id().into());

    for metric in Stat::iter() {
        metrics.add_output(&metric, Output::Reading);
        let _ = metrics.record_counter(&metric, Instant::now(), 0);
    }

    metrics
}
