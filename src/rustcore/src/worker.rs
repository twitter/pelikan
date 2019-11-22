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

use tokio::prelude::*;
use tokio::runtime::current_thread::spawn;
use tokio::sync::mpsc::Receiver;

use ccommon::buf::OwnedBuf;
use ccommon::{metric::*, Metrics};
use ccommon_sys::{buf, buf_sock_borrow, buf_sock_return};
use pelikan::protocol::{PartialParseError, Protocol, StatefulProtocol};

use std::io::Result;
use std::rc::Rc;

use crate::{Action, ClosableStream, Worker};

type RequestState<H> = <<H as Worker>::Protocol as StatefulProtocol>::RequestState;
type ResponseState<H> = <<H as Worker>::Protocol as StatefulProtocol>::ResponseState;

#[allow(clippy::too_many_arguments)]
async fn read_once<'a, W, S>(
    worker: &'a Rc<W>,
    stream: &'a mut S,
    metrics: &'static WorkerMetrics,
    wbuf: &'a mut OwnedBuf,
    rbuf: &'a mut OwnedBuf,
    req_st: &'a mut RequestState<W>,
    rsp_st: &'a mut ResponseState<W>,
    state: &'a mut W::State,
) -> std::result::Result<(), ()>
where
    W: Worker,
    S: AsyncRead + AsyncWrite + Unpin,
{
    match crate::buf::read_buf(stream, rbuf).await {
        Ok(0) => {
            if rbuf.write_size() == 0 {
                metrics.socket_read.incr();
                // If this fails then just close the connection,
                // there isn't really anything we can do otherwise.
                return rbuf.fit(rbuf.read_size() + 1024).map_err(|e| {
                    error!("Failed to resize read buffer: {}", e);
                });
            } else {
                // This can occurr when a the other end of the connection
                // disappears. At this point we can just close the connection
                // as otherwise we will continuously read 0 and waste CPU
                return Err(());
            }
        }
        Ok(nbytes) => {
            metrics.bytes_read.incr_n(nbytes as u64);
            metrics.socket_read.incr();
        }
        Err(_) => {
            metrics.socket_read_ex.incr();
            return Err(());
        }
    };

    while rbuf.read_size() > 0 {
        let req = match W::Protocol::parse_req(req_st, rbuf) {
            Ok(req) => req,
            Err(e) => {
                if e.is_unfinished() {
                    break;
                }

                metrics.request_parse_ex.incr();
                return Err(());
            }
        };

        let rsp = match worker.process_request(req, rsp_st, state) {
            Action::Respond(rsp) => rsp,
            Action::Close => return Err(()),
            Action::NoResponse => continue,
            Action::__Nonexhaustive(empty) => match empty {},
        };

        W::Protocol::compose_rsp(rsp, rsp_st, wbuf).map_err(|_| {
            metrics.response_compose_ex.incr();
        })?;

        while wbuf.read_size() > 0 {
            let nbytes = crate::buf::write_buf(stream, wbuf).await.map_err(|_| {
                metrics.socket_write_ex.incr();
            })?;

            metrics.socket_write.incr();
            metrics.bytes_sent.incr_n(nbytes as u64);
        }

        wbuf.lshift();
    }

    rbuf.lshift();

    Ok(())
}

async fn worker_driver<W, S>(worker: Rc<W>, mut stream: S, metrics: &'static WorkerMetrics)
where
    W: Worker,
    S: AsyncRead + AsyncWrite + Unpin + ClosableStream,
{
    // Variable we use to constrain the lifetime of rbuf and wbuf
    let mut sock = unsafe { buf_sock_borrow() };
    let (rbuf, wbuf) = unsafe {
        (
            &mut *(&mut (*sock).wbuf as *mut *mut buf as *mut OwnedBuf),
            &mut *(&mut (*sock).rbuf as *mut *mut buf as *mut OwnedBuf),
        )
    };

    let mut req = RequestState::<W>::default();
    let mut rsp = ResponseState::<W>::default();
    let mut state = Default::default();

    while let Ok(_) = read_once(
        &worker,
        &mut stream,
        metrics,
        wbuf,
        rbuf,
        &mut req,
        &mut rsp,
        &mut state,
    )
    .await
    {}

    // Best-effort attempt to close the stream - if it doesn't
    // close then there's nothing that we can really do here.
    // Note: If a read from the socket already failed then it's
    //       probable that closing the stream would fail too.
    let _ = stream.close();
    metrics.active_conns.decr();

    unsafe {
        buf_sock_return(&mut sock as *mut _);
    }
}

/// Process an incoming stream of new connections and spin up
/// a future that processes each of them.
pub async fn worker<W, S>(
    mut chan: Receiver<S>,
    worker: Rc<W>,
    metrics: &'static WorkerMetrics,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + ClosableStream + 'static,
    W: Worker + 'static,
{
    loop {
        let stream: S = match chan.recv().await {
            Some(stream) => stream,
            None => {
                info!("All acceptors have shut down. Shutting down the worker!");
                // All upstream senders are closed so we might
                // as well exit.
                break;
            }
        };

        info!("Accepted new connection!");

        metrics.active_conns.incr();

        spawn(worker_driver(Rc::clone(&worker), stream, metrics))
    }

    Ok(())
}

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
