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
use std::io::Result;
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

/// Used to contrain an unbounded lifetime produced by
/// a pointer dereference.
fn constrain_lifetime<'a, A, B>(x: &'a mut A, _: &'a B) -> &'a mut A {
    x
}

/// Process all the new bytes that were just read.
async fn read_once<H, S>(
    handler: &Rc<RefCell<H>>,
    stream: &mut S,
    wbuf: &mut OwnedBuf,
    rbuf: &mut OwnedBuf,
    req: &mut Request<H>,
    rsp: &mut Response<H>,
) -> std::result::Result<(), ()>
where
    H: AdminHandler + 'static,
    S: AsyncWrite + AsyncRead + Unpin,
    <H::Protocol as Protocol>::Request: QuitRequest,
{
    match crate::buf::read_buf(stream, rbuf).await {
        Ok(0) => {
            if rbuf.write_size() == 0 {
                // If this fails then just close the connection,
                // there isn't really anything we can do otherwise.
                return rbuf.fit(rbuf.read_size() + 1024).map_err(|_| ());
            } else {
                // This can occurr when a the other end of the connection
                // disappears. At this point we can just close the connection
                // as otherwise we will continuously read 0 and waste CPU
                return Err(());
            }
        }
        Ok(_) => (),
        Err(_) => return Err(()),
    };

    while rbuf.read_size() > 0 {
        if let Err(e) = req.parse(rbuf) {
            if e.is_unfinished() {
                req.reset();
                break;
            }

            return Err(());
        }

        if req.is_quit() {
            info!("Admin peer called quit");
            return Err(());
        }

        handler.borrow_mut().process_request(rsp, req);

        if let Err(e) = rsp.compose(wbuf) {
            error!("Failed to compose admin response: {}", e);
            return Err(());
        }

        while wbuf.read_size() > 0 {
            // If this fails then something went wrong with the buffer and
            // we can't write anything to it. Probably means that the
            // connection is dead so just close it.
            crate::buf::write_buf(stream, wbuf).await.map_err(|_| ())?;
        }

        // wbuf is definitely empty here but need to reset the pointers
        // to the start of the buffer.
        wbuf.lshift();

        rsp.reset();
        req.reset();
    }

    rbuf.lshift();

    Ok(())
}

/// Process a single request stream
async fn admin_tcp_stream_handler<H, S>(handler: Rc<RefCell<H>>, mut stream: S)
where
    H: AdminHandler + 'static,
    S: AsyncWrite + AsyncRead + Unpin,
    <H::Protocol as Protocol>::Request: QuitRequest,
{
    // Variable we use to constrain the lifetime of rbuf and wbuf
    let dummy = ();
    let mut sock = unsafe { buf_sock_borrow() };
    let (rbuf, wbuf) = unsafe {
        (
            constrain_lifetime(
                &mut *(&mut (*sock).wbuf as *mut *mut buf as *mut OwnedBuf),
                &dummy,
            ),
            constrain_lifetime(
                &mut *(&mut (*sock).rbuf as *mut *mut buf as *mut OwnedBuf),
                &dummy,
            ),
        )
    };

    let mut req = Request::<H>::default();
    let mut rsp = Response::<H>::default();

    while let Ok(()) = read_once(&handler, &mut stream, wbuf, rbuf, &mut req, &mut rsp).await {}

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
