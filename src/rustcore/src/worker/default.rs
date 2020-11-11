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

use std::rc::Rc;

use ccommon::buf::OwnedBuf;
use pelikan::protocol::{PartialParseError, Protocol};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::util::{read_buf, write_buf, BufIOError};
use crate::{Action, Worker, WorkerMetrics};

type Request<H> = <<H as Worker>::Protocol as Protocol>::Request;
type Response<H> = <<H as Worker>::Protocol as Protocol>::Response;

/// Worker instance that makes use of the [`Worker`][0] trait.
///
/// [0]: crate::Worker
pub async fn default_worker<'a, W, S>(
    worker: Rc<W>,
    stream: &'a mut S,
    rbuf: &'a mut OwnedBuf,
    wbuf: &'a mut OwnedBuf,
    metrics: &'static WorkerMetrics,
) where
    W: Worker + 'static,
    S: AsyncRead + AsyncWrite + Unpin + 'static,
{
    let mut req = Request::<W>::default();
    let mut rsp = Response::<W>::default();
    let mut state = Default::default();

    loop {
        if let Err(e) = read_buf(stream, rbuf, metrics).await {
            match e {
                BufIOError::StreamClosed | BufIOError::IOError(_) => return,
                e => {
                    warn!("Failed to read from stream: {}", e);
                    return;
                }
            }
        }

        while rbuf.read_size() > 0 {
            if let Err(e) = W::Protocol::parse_req(&mut req, rbuf) {
                if e.is_unfinished() {
                    break;
                }

                metrics.request_parse_ex.incr();
                return;
            };

            match worker.process_request(&mut req, &mut rsp, &mut state) {
                Action::Respond => (),
                Action::Close => return,
                Action::NoResponse => continue,
                Action::__Nonexhaustive(empty) => match empty {},
            };

            if let Err(_) = W::Protocol::compose_rsp(&mut rsp, wbuf) {
                metrics.response_compose_ex.incr();
                return;
            }

            if let Err(_) = write_buf(stream, wbuf, metrics).await {
                return;
            }
        }
    }
}
