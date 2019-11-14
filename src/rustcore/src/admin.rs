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

use pelikan::core::admin::AdminHandler;
use pelikan::protocol::{PartialParseError, Protocol, QuitRequest, Serializable};

use std::cell::RefCell;
use std::io::{Result, Write};
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::runtime::current_thread::spawn;
use tokio::timer::Interval;

// Uncomment once tokio updates to 0.2.0-alpha.7
// use tokio::signal::CtrlC;
// use futures::select;

use ccommon::buf::OwnedBuf;
use ccommon_sys::{buf, buf_sock_borrow, buf_sock_return};

type Request<H> = <<H as AdminHandler>::Protocol as Protocol>::Request;
type Response<H> = <<H as AdminHandler>::Protocol as Protocol>::Response;

#[inline(never)]
fn assert_buf_valid(buf: &mut OwnedBuf) {
    unsafe {
        let ptr: *mut *mut buf = std::mem::transmute(buf);

        let mut value = true;
        value = value && (**ptr).rpos as usize <= (**ptr).end as usize;
        value = value && (**ptr).wpos as usize <= (**ptr).end as usize;
        value = value && (**ptr).rpos as usize <= (**ptr).wpos as usize;

        if !value {
            panic!();
        }
    }
}

/// Process a single request stream
async fn admin_tcp_stream_handler<H, S>(handler: Rc<RefCell<H>>, mut stream: S)
where
    H: AdminHandler + 'static,
    S: AsyncWrite + AsyncRead + Unpin,
    <H::Protocol as Protocol>::Request: QuitRequest,
{
    let mut sock = unsafe { buf_sock_borrow() };

    let (mut rbuf, mut wbuf) = unsafe {
        (
            OwnedBuf::from_raw((*sock).wbuf),
            OwnedBuf::from_raw((*sock).rbuf),
        )
    };

    let mut tmpbuf = [0u8; 1024];

    let mut req = Request::<H>::default();
    let mut rsp = Response::<H>::default();

    // let ctrlc = CtrlC::new();

    'outer: loop {
        let nbytes = match stream.read(&mut tmpbuf).await {
            Ok(nbytes) => nbytes,
            Err(_) => break,
        };

        // Uncomment this once tokio updates to 0.2.0-alpha.7
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
        };

        while rbuf.read_size() > 0 {
            if let Err(e) = req.parse(&mut rbuf) {
                if e.is_unfinished() {
                    break;
                }
            };

            // Since we want to remain consistent with core we don't
            // do anything once we've recieved the quit message.
            if req.is_quit() {
                info!("Admin peer called quit");
                break 'outer;
            }

            let mut borrow = handler.borrow_mut();
            borrow.process_request(&mut rsp, &mut req);
            // Need to ensure that borrow doesn't live across a
            // suspend point as otherwise we could panic if another
            // task tries to borrow it.
            drop(borrow);

            assert_buf_valid(&mut wbuf);

            if let Err(e) = rsp.compose(&mut wbuf) {
                error!("Failed to compose admin response: {}", e);
                break 'outer;
            }

            if wbuf.read_size() > 0 {
                if let Err(_) = stream.write_all(wbuf.as_slice()).await {
                    // Something went wrong with the buffer and we can't
                    // write anything to it. Probably means that the connection
                    // is dead so just close it.
                    break 'outer;
                }

                // Need to reset every time otherwise we'll resend existing
                // messages for the next request.
                let _ = wbuf.reset();
            }

            rsp.reset();
        }

        req.reset();
        rbuf.lshift();
    }

    // We don't own wbuf or rbuf so don't drop them.
    // (dropping them during a panic is fine since buf_sock_return isn't called)
    std::mem::forget(wbuf);
    std::mem::forget(rbuf);

    unsafe {
        buf_sock_return(&mut sock as *mut _);
    }
}

async fn flush_debug_log(duration: Duration) {
    use ccommon_sys::debug_log_flush;

    let mut intvl = Interval::new(Instant::now(), duration);

    loop {
        let _ = intvl.next().await;

        unsafe {
            debug_log_flush(std::ptr::null_mut());
        }
    }
}

/// Listens for requests on the admin port
/// using TCP.
pub async fn admin_tcp<H: AdminHandler + 'static>(
    addr: SocketAddr,
    handler: H,
    log_flush_interval: Duration,
) -> Result<()>
where
    <H::Protocol as Protocol>::Request: QuitRequest,
{
    let mut listener = TcpListener::bind(addr).await?;
    let handler = Rc::new(RefCell::new(handler));

    spawn(flush_debug_log(log_flush_interval));

    loop {
        let stream: TcpStream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(e) => {
                error!("Failed to establish connection: {}", e);
                continue;
            }
        };

        spawn(admin_tcp_stream_handler(Rc::clone(&handler), stream));
    }
}
