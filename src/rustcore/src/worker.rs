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

// Uncomment once tokio updates to 0.2.0-alpha.7
// use tokio::signal::CtrlC;
// use futures::select;

use ccommon::buf::OwnedBuf;
use ccommon_sys::{buf_sock_borrow, buf_sock_return};
use pelikan::core::DataProcessor;

use std::cell::RefCell;
use std::io::{Result, Write};
use std::rc::Rc;

use crate::WorkerMetrics;

async fn worker_conn_driver<P, S>(
    dp: Rc<RefCell<P>>,
    mut stream: S,
    metrics: &'static WorkerMetrics,
) where
    P: DataProcessor,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut state: Option<&mut P::SockState> = None;
    let mut sock = unsafe { buf_sock_borrow() };
    let (mut rbuf, mut wbuf) = unsafe {
        (
            OwnedBuf::from_raw((*sock).wbuf),
            OwnedBuf::from_raw((*sock).rbuf),
        )
    };

    // let mut ctrlc = CtrlC::new();

    let mut tmpbuf = [0u8; 1024];
    'outer: loop {
        let nbytes = match stream.read(&mut tmpbuf).await {
            Ok(0) => {
                // This can occurr when a the other end of the connection
                // disappears. At this point we can just close the connection
                // as otherwise we will continuously read 0 and waste CPU
                break
            },
            Ok(nbytes) => nbytes,
            Err(_) => break
        };

        info!("Read {} bytes from stream", nbytes);

        // Uncomment once we update to tokio 0.2.0-alpha.7
        // let nbytes = select! {
        //     res = stream.read(&mut tmpbuf).fuse() => match res {
        //         Ok(nbytes) => nbytes,
        //         Err(_) => break 'outer
        //     },
        //     _ = ctrlc => break 'outer
        // };

        let _ = rbuf.fit(rbuf.read_size() + tmpbuf.len());
        match rbuf.write_all(&tmpbuf[..nbytes]) {
            Ok(()) => (),
            Err(e) => {
                error!("Failed to expand buffer: {}", e);
                // There really isn't anything we can do to recover the
                // connection from this state since we just truncated
                // part of a message. Instead, we just close the connection.
                break;
            }
        }

        metrics.socket_read.incr();
        metrics.bytes_read.incr_n(nbytes as u64);

        let mut prev_size = std::usize::MAX;
        // Don't iterate until the packet is empty, there may be
        // a partial response that got fragmented.
        while rbuf.read_size() < prev_size {
            prev_size = rbuf.read_size();
            let mut borrow = dp.borrow_mut();
            if let Err(_) = borrow.read(&mut rbuf, &mut wbuf, &mut state) {
                metrics.socket_write_ex.incr();
                // Unable to read the socket. This should only occur when
                // the peer closed the socket or was lost.
                break 'outer;
            }
            // Don't want borrow living across a suspend point
            drop(borrow);

            if wbuf.read_size() > 0 {
                if let Err(_) = stream.write_all(wbuf.as_slice()).await {
                    metrics.socket_write_ex.incr();
                    // Something went wrong with the buffer and we can't
                    // write anything to it. Probably means that the connection
                    // is dead so just close it.
                    break 'outer;
                }

                metrics.bytes_sent.incr_n(wbuf.read_size() as u64);
                metrics.socket_write.incr();

                let mut borrow = dp.borrow_mut();
                if let Err(_) = borrow.write(&mut rbuf, &mut wbuf, &mut state) {
                    break 'outer;
                }
            }
        }

        rbuf.lshift();
    }

    let mut borrow = dp.borrow_mut();
    // We're already exiting, ignore any errors
    let _ = borrow.error(&mut rbuf, &mut wbuf, &mut state);

    metrics.active_conns.decr();

    // We don't own wbuf or rbuf so don't drop them.
    // (dropping them during a panic is fine since buf_sock_return isn't called)
    std::mem::forget(wbuf);
    std::mem::forget(rbuf);

    unsafe {
        buf_sock_return(&mut sock as *mut _);
    }
}

pub async fn worker<P, S>(
    mut chan: Receiver<S>,
    dp: P,
    metrics: &'static WorkerMetrics,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + 'static,
    P: DataProcessor + 'static,
{
    let dp = Rc::new(RefCell::new(dp));

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

        spawn(worker_conn_driver(Rc::clone(&dp), stream, metrics))
    }

    Ok(())
}
