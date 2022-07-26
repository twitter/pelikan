// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::protocol::*;
use crate::*;

pub(crate) async fn handle_memcache_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::with_capacity(INITIAL_BUFFER_SIZE);

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

pub(crate) async fn handle_redis_client(
    mut socket: tokio::net::TcpStream,
    mut client: SimpleCacheClient,
    cache_name: String,
) {
    // initialize a buffer for incoming bytes from the client
    let mut buf = Buffer::with_capacity(INITIAL_BUFFER_SIZE);

    // initialize the request parser
    let parser = redis::RequestParser {};

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
                    redis::Request::Get(r) => {
                        if redis::get(&mut client, &cache_name, &mut socket, r.key())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    redis::Request::Set(r) => {
                        if redis::set(&mut client, &cache_name, &mut socket, &r)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                buf.consume(consumed);
            }
            Err(ParseError::Incomplete) => {}
            Err(ParseError::Invalid) => {
                // invalid request
                println!("bad request");
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
            Err(ParseError::Unknown) => {
                // unknown command
                println!("unknown command");
                let _ = socket.write_all(b"CLIENT_ERROR\r\n").await;
                break;
            }
        }
    }
}
