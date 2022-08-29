// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub(crate) async fn listener(
    listener: TcpListener,
    client_builder: SimpleCacheClientBuilder,
    cache_name: String,
    protocol: Protocol,
) {
    // this acts as our listener thread and spawns tasks for each client
    loop {
        // accept a new client
        if let Ok((socket, _)) = listener.accept().await {
            TCP_ACCEPT.increment();

            let client = client_builder.clone().build();
            let cache_name = cache_name.clone();

            // spawn a task for managing requests for the client
            tokio::spawn(async move {
                TCP_CONN_CURR.increment();
                match protocol {
                    Protocol::Memcache => {
                        crate::frontend::handle_memcache_client(socket, client, cache_name).await;
                    }
                    Protocol::Resp => {
                        crate::frontend::handle_resp_client(socket, client, cache_name).await;
                    }
                }

                TCP_CLOSE.increment();
                TCP_CONN_CURR.decrement();
            });
        }
    }
}
