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

use std::io::Result;
use std::net::{Shutdown, SocketAddr};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{error::TrySendError, Sender};

use ccommon::{metric::*, Metrics};

pub async fn tcp_listener(
    addr: SocketAddr,
    mut chan: Sender<TcpStream>,
    metrics: &'static TcpListenerMetrics,
) -> Result<()> {
    let mut listener = TcpListener::bind(addr).await?;

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

        let _ = stream.set_nodelay(true);

        if let Err(e) = chan.try_send(stream) {
            let (val, is_closed) = match e {
                TrySendError::Full(val) => (val, false),
                TrySendError::Closed(val) => (val, true),
            };

            // Gracefully close the channel if possible
            let _ = val.shutdown(Shutdown::Both);

            if is_closed {
                info!("Channel has shut down, shutting down TCP listener.");
                break;
            }

            metrics.queue_full_ex.incr();

            error!("New connection queue is full. Dropping a connection!");
        }
    }

    Ok(())
}

#[derive(Metrics)]
#[repr(C)]
pub struct TcpListenerMetrics {
    #[metric(name = "tcp_accept_ex", desc = "# of times that accept failed")]
    pub tcp_accept_ex: Counter,
    #[metric(
        name = "tcp_accept",
        desc = "# of connections that have been accepted by the TCP acceptor"
    )]
    pub tcp_accept: Counter,
    #[metric(
        name = "tcp_accept_queue_full_ex",
        desc = "# of connections dropped because the queue was full"
    )]
    pub queue_full_ex: Counter,
}
