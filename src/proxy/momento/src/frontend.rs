// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::protocol::*;
use crate::*;
use session::Buf;

pub(crate) async fn handle_memcache_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::new(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = memcache::RequestParser::new();

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
                    memcache::Request::Get(r) => {
                        if memcache::get(&mut client, &cache_name, &mut socket, r.keys())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    memcache::Request::Set(r) => {
                        if memcache::set(&mut client, &cache_name, &mut socket, &r)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ => {
                        debug!("unsupported command: {}", request);
                    }
                }
                buf.advance(consumed);
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {}
                _ => {
                    // invalid request
                    let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                    break;
                }
            },
        }
    }
}

pub(crate) async fn handle_resp_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::new(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = resp::RequestParser::new();

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
                    resp::Request::Get(r) => {
                        if resp::get(&mut client, &cache_name, &mut socket, r.key())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    resp::Request::Set(r) => {
                        if resp::set(&mut client, &cache_name, &mut socket, &r)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                buf.advance(consumed);
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {}
                _ => {
                    println!("bad request");
                    let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                    break;
                }
            },
        }
    }
}
