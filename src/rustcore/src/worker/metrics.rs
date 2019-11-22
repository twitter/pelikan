use ccommon::metric::*;

/// Metrics collected by a worker.
#[derive(Metrics)]
#[repr(C)]
pub struct WorkerMetrics {
    #[metric(
        name = "worker_socket_read",
        desc = "# of times that a worker has read from a socket"
    )]
    pub socket_read: Counter,
    #[metric(
        name = "worker_socket_write",
        desc = "# of times that a worker has written to a socket"
    )]
    pub socket_write: Counter,
    #[metric(name = "worker_active_conns", desc = "# of active connections")]
    pub active_conns: Gauge,
    #[metric(
        name = "worker_bytes_read",
        desc = "# of bytes that the worker has recieved"
    )]
    pub bytes_read: Counter,
    #[metric(
        name = "worker_bytes_sent",
        desc = "# of bytes sent by the worker thread"
    )]
    pub bytes_sent: Counter,
    #[metric(
        name = "worker_socket_read_ex",
        desc = "# of times that a socket read has failed"
    )]
    pub socket_read_ex: Counter,
    #[metric(
        name = "worker_socket_write_ex",
        desc = "# of times that a socket write has failed"
    )]
    pub socket_write_ex: Counter,
    #[metric(
        name = "worker_request_parse_ex",
        desc = "# of times that an incoming request failed to parse"
    )]
    pub request_parse_ex: Counter,
    #[metric(
        name = "worker_response_compose_ex",
        desc = "# of times that an outgoing response failed to parse"
    )]
    pub response_compose_ex: Counter,
}
