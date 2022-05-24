// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::commands::*;
use crate::*;

pub(crate) async fn handle_proxy_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::with_capacity(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = MemcacheRequestParser::new(MAX_REQUEST_SIZE, TIME_TYPE);

    // handle incoming data from the client
    loop {
        if do_read(&mut socket, &mut buf).await.is_err() {
            break;
        }

        match parser.parse(buf.borrow()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();

                match request {
                    MemcacheRequest::Get { keys, .. } => {
                        if get(&mut client, &cache_name, &mut socket, &keys)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    MemcacheRequest::Set { entry, noreply } => {
                        if set(&mut client, &cache_name, &mut socket, entry, noreply)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ => {
                        debug!("unsupported command: {}", request.command());
                    }
                }
                buf.consume(consumed);
            }
            Err(ParseError::Incomplete) => {}
            Err(ParseError::Invalid) => {
                // invalid request
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
            Err(ParseError::Unknown) => {
                // unknown command
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
        }
    }
}
