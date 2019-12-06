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
use ccommon_sys::{buf, buf_sock_borrow, buf_sock_return};
use tokio::sync::mpsc::Receiver;
use tokio::task::spawn_local;

use crate::traits::WorkerFn;
use crate::{ClosableStream, WorkerMetrics};

/// Handler that sets up the event loop for each new connection.
pub(crate) async fn worker<W, S, F>(
    mut chan: Receiver<S>,
    state: Rc<W>,
    metrics: &'static WorkerMetrics,
    worker: F,
) where
    F: (for<'a> WorkerFn<'a, W, S>) + 'static,
    W: 'static,
    S: ClosableStream + 'static,
{
    let worker = Rc::new(worker);

    while let Some(mut stream) = chan.recv().await {
        metrics.active_conns.incr();

        let worker = Rc::clone(&worker);
        let state = Rc::clone(&state);
        spawn_local(async move {
            // Variable we use to constrain the lifetime of rbuf and wbuf
            let mut sock = unsafe { buf_sock_borrow() };
            let (rbuf, wbuf) = unsafe {
                (
                    &mut *(&mut (*sock).wbuf as *mut *mut buf as *mut OwnedBuf),
                    &mut *(&mut (*sock).rbuf as *mut *mut buf as *mut OwnedBuf),
                )
            };

            // Run the actual worker thread
            worker.eval(state, &mut stream, rbuf, wbuf, metrics).await;

            // Best-effort attempt to close the stream - if it doesn't
            // close then there's nothing that we can really do here.
            // Note: If a read from the socket already failed then it's
            //       probable that closing the stream would fail too.
            let _ = stream.close();
            metrics.active_conns.decr();

            unsafe {
                buf_sock_return(&mut sock as *mut _);
            }
        });
    }
}
