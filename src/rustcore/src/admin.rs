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

use pelikan::protocol::{PartialParseError, Protocol, Resettable};

use std::cell::RefCell;
use std::ffi::CStr;
use std::io::Result;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::runtime::current_thread::spawn;
use tokio::timer::Interval;

use crate::errors::{AddrParseData, AddrParseError};
use crate::{Action, AdminHandler, ClosableStream};

use ccommon::buf::OwnedBuf;
use ccommon::{metric::*, option::*, Metrics, Options};
use ccommon_sys::{buf, buf_sock_borrow, buf_sock_return};

type Request<H> = <<H as AdminHandler>::Protocol as Protocol>::Request;
type Response<H> = <<H as AdminHandler>::Protocol as Protocol>::Response;

/// Process all the new bytes that were just read.
#[allow(clippy::too_many_arguments)]
async fn read_once<H, S>(
    handler: &Rc<RefCell<H>>,
    stream: &mut S,
    wbuf: &mut OwnedBuf,
    rbuf: &mut OwnedBuf,
    req: &mut Request<H>,
    rsp: &mut Response<H>,
    _metrics: &AdminMetrics,
) -> std::result::Result<(), ()>
where
    H: AdminHandler + 'static,
    S: AsyncWrite + AsyncRead + Unpin,
{
    match crate::util::ReadBuf::new(stream, rbuf).await {
        Ok(0) => {
            if rbuf.write_size() == 0 {
                // If this fails then just close the connection,
                // there isn't really anything we can do otherwise.
                return rbuf.double().map_err(|_| ());
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
        if let Err(e) = H::Protocol::parse_req(req, rbuf) {
            if e.is_unfinished() {
                req.reset();
                break;
            }

            return Err(());
        };

        match handler.borrow_mut().process_request(req, rsp) {
            Action::Respond => (),
            Action::Close => return Err(()),
            Action::NoResponse => continue,
            Action::__Nonexhaustive(empty) => match empty {},
        };

        if let Err(e) = H::Protocol::compose_rsp(rsp, wbuf) {
            error!("Failed to compose admin response: {}", e);
            return Err(());
        }

        while wbuf.read_size() > 0 {
            // If this fails then something went wrong with the buffer and
            // we can't write anything to it. Probably means that the
            // connection is dead so just close it.
            crate::util::WriteBuf::new(stream, wbuf)
                .await
                .map_err(|_| ())?;
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
async fn admin_tcp_stream_handler<H, S>(
    handler: Rc<RefCell<H>>,
    mut stream: S,
    metrics: &'static AdminMetrics,
) where
    H: AdminHandler + 'static,
    S: AsyncWrite + AsyncRead + ClosableStream + Unpin,
{
    metrics.active_conns.incr();

    let mut sock = unsafe { buf_sock_borrow() };
    let (rbuf, wbuf) = unsafe {
        (
            &mut *(&mut (*sock).wbuf as *mut *mut buf as *mut OwnedBuf),
            &mut *(&mut (*sock).rbuf as *mut *mut buf as *mut OwnedBuf),
        )
    };

    let mut req = Request::<H>::default();
    let mut rsp = Response::<H>::default();

    while let Ok(()) = read_once(
        &handler,
        &mut stream,
        wbuf,
        rbuf,
        &mut req,
        &mut rsp,
        metrics,
    )
    .await
    {}

    // Best-effort attempt to close the socket - if this fails then
    // we can't really do anything anyway so ignore the error.
    // Note: If a read from the socket already failed then it's
    //       probable that closing the stream would fail too.
    let _ = stream.close();

    metrics.active_conns.decr();

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
    metrics: &'static AdminMetrics,
) -> Result<()> {
    let mut listener = TcpListener::bind(addr).await?;
    let handler = Rc::new(RefCell::new(handler));

    spawn(flush_debug_log(log_flush_interval));

    loop {
        let stream: TcpStream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(e) => {
                metrics.tcp_accept_ex.incr();

                debug!("Failed to accept connection: {}", e);
                continue;
            }
        };

        metrics.tcp_accept.incr();

        spawn(admin_tcp_stream_handler(
            Rc::clone(&handler),
            stream,
            metrics,
        ));
    }
}

#[derive(Options)]
#[repr(C)]
pub struct AdminOptions {
    #[option(desc = "admin interface", default = std::ptr::null_mut())]
    pub admin_host: Str,
    #[option(desc = "admin port", default = 9999)]
    pub admin_port: UInt,
    #[option(desc = "debug log flush interval (ms)", default = 500)]
    pub dlog_intvl: UInt,
}

impl AdminOptions {
    fn _addr(&self) -> std::result::Result<SocketAddr, AddrParseData> {
        let ptr = self.admin_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.admin_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseData::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }

    pub fn addr(&self) -> std::result::Result<SocketAddr, AddrParseError> {
        self._addr().map_err(AddrParseError)
    }

    pub fn dlog_intvl(&self) -> Duration {
        Duration::from_millis(self.dlog_intvl.value())
    }
}

#[derive(Metrics)]
#[repr(C)]
pub struct AdminMetrics {
    #[metric(
        name = "admin_tcp_accept_ex",
        desc = "# of times that an admin TCP accept failed"
    )]
    pub tcp_accept_ex: Counter,
    #[metric(
        name = "admin_tcp_accept",
        desc = "# of times that a connection was accepted on admin TCP port"
    )]
    pub tcp_accept: Counter,
    #[metric(
        name = "admin_active_conns",
        desc = "# of currently open connections on the admin port"
    )]
    pub active_conns: Gauge,
}
