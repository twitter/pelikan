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
use tokio::sync::mpsc::Sender;

use crate::TcpAcceptorMetrics;

pub async fn tcp_listener(
    addr: SocketAddr,
    mut chan: Sender<TcpStream>,
    metrics: &'static TcpAcceptorMetrics,
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
            if e.is_closed() {
                info!("Channel has shut down, shutting down TCP listener.");
                break;
            }

            metrics.queue_full_ex.incr();

            let val = e.into_inner();
            // Gracefully close the channel if possible
            let _ = val.shutdown(Shutdown::Both);

            error!("New connection queue is full. Dropping a connection!");
        }
    }

    Ok(())
}
