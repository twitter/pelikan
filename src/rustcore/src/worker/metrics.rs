// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
