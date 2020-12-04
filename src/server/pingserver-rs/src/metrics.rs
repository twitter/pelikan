// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_metrics::*;

use std::sync::Arc;
use std::time::Instant;

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
        match self {
            Stat::AdminEventError => "admin_event_error",
            Stat::AdminEventLoop => "admin_event_loop",
            Stat::AdminEventRead => "admin_event_read",
            Stat::AdminEventTotal => "admin_event_total",
            Stat::AdminEventWrite => "admin_event_write",
            Stat::AdminRequestParse => "admin_request_parse",
            Stat::AdminRequestParseEx => "admin_request_parse_ex",
            Stat::AdminResponseCompose => "admin_response_compose",
            Stat::AdminResponseComposeEx => "admin_response_compose_ex",
            Stat::Pid => "pid",
            Stat::RequestParse => "request_parse",
            Stat::RequestParseEx => "request_parse_ex",
            Stat::ResponseCompose => "response_compose",
            Stat::ResponseComposeEx => "response_compose_ex",
            Stat::ServerEventError => "server_event_error",
            Stat::ServerEventLoop => "server_event_loop",
            Stat::ServerEventRead => "server_event_read",
            Stat::ServerEventTotal => "server_event_total",
            Stat::ServerEventWrite => "server_event_write",
            Stat::SessionRecv => "session_recv",
            Stat::SessionRecvByte => "session_recv_byte",
            Stat::SessionRecvEx => "session_recv_ex",
            Stat::SessionSend => "session_send",
            Stat::SessionSendByte => "session_send_byte",
            Stat::SessionSendEx => "session_send_ex",
            Stat::TcpAccept => "tcp_accept",
            Stat::TcpAcceptEx => "tcp_accept_ex",
            Stat::TcpClose => "tcp_close",
            Stat::TcpConnect => "tcp_connect",
            Stat::TcpConnectEx => "tcp_connect_ex",
            Stat::TcpRecv => "tcp_recv",
            Stat::TcpRecvByte => "tcp_recv_byte",
            Stat::TcpRecvEx => "tcp_recv_ex",
            Stat::TcpReject => "tcp_reject",
            Stat::TcpRejectEx => "tcp_reject_ex",
            Stat::TcpSend => "tcp_send",
            Stat::TcpSendByte => "tcp_send_byte",
            Stat::TcpSendEx => "tcp_send_ex",
            Stat::WorkerEventError => "worker_event_error",
            Stat::WorkerEventLoop => "worker_event_loop",
            Stat::WorkerEventRead => "worker_event_read",
            Stat::WorkerEventTotal => "worker_event_total",
            Stat::WorkerEventWake => "worker_event_wake",
            Stat::WorkerEventWrite => "worker_event_write",
        }
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

    for metric in &[
        Stat::AdminEventError,
        Stat::AdminEventLoop,
        Stat::AdminEventRead,
        Stat::AdminEventTotal,
        Stat::AdminEventWrite,
        Stat::AdminRequestParse,
        Stat::AdminRequestParseEx,
        Stat::AdminResponseCompose,
        Stat::AdminResponseComposeEx,
        Stat::RequestParse,
        Stat::RequestParseEx,
        Stat::ResponseCompose,
        Stat::ResponseComposeEx,
        Stat::ServerEventError,
        Stat::ServerEventLoop,
        Stat::ServerEventRead,
        Stat::ServerEventTotal,
        Stat::ServerEventWrite,
        Stat::SessionRecv,
        Stat::SessionRecvByte,
        Stat::SessionRecvEx,
        Stat::SessionSend,
        Stat::SessionSendByte,
        Stat::SessionSendEx,
        Stat::TcpAccept,
        Stat::TcpAcceptEx,
        Stat::TcpClose,
        Stat::TcpConnect,
        Stat::TcpConnectEx,
        Stat::TcpRecv,
        Stat::TcpRecvByte,
        Stat::TcpRecvEx,
        Stat::TcpReject,
        Stat::TcpRejectEx,
        Stat::TcpSend,
        Stat::TcpSendByte,
        Stat::TcpSendEx,
        Stat::WorkerEventError,
        Stat::WorkerEventLoop,
        Stat::WorkerEventRead,
        Stat::WorkerEventTotal,
        Stat::WorkerEventWake,
        Stat::WorkerEventWrite,
    ] {
        metrics.add_output(metric, Output::Reading);
        let _ = metrics.record_counter(metric, Instant::now(), 0);
    }

    metrics
}
