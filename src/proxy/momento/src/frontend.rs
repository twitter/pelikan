// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::commands::*;
use crate::*;
use buffer::Buf;

pub(crate) async fn handle_proxy_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::new(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = RequestParser::new().max_value_size(MAX_REQUEST_SIZE);

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
                    Request::Get(request) => {
                        if get(&mut client, &cache_name, &mut socket, request)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Request::Set(request) => {
                        if set(&mut client, &cache_name, &mut socket, request)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    request => {
                        debug!("unsupported command: {}", request);
                    }
                }
                buf.advance(consumed);
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {},
                _ => {
                    // invalid request
                    let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                    break;
                }
            }
        }
    }
}
