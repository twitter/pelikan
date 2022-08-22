use crate::klog::klog_set;
use crate::*;

pub async fn set(
    client: &mut SimpleCacheClient,
    cache_name: &str,
    socket: &mut tokio::net::TcpStream,
    request: &protocol_memcache::Set,
) -> Result<(), Error> {
    SET.increment();

    if let Ok(key) = std::str::from_utf8(&request.key()) {
        let value = if let Ok(value) = std::str::from_utf8(&request.value()) {
            value.to_owned()
        } else {
            debug!("value is not valid utf8: {:?}", request.value());
            let _ = socket.write_all(b"ERROR\r\n").await;
            return Err(Error::from(ErrorKind::InvalidInput));
        };

        if value.len() == 0 {
            error!("empty values are not supported by momento");
            SESSION_SEND.increment();
            SESSION_SEND_BYTE.add(7);
            TCP_SEND_BYTE.add(7);
            if socket.write_all(b"ERROR\r\n").await.is_err() {
                SESSION_SEND_EX.increment();
            }
            return Err(Error::from(ErrorKind::InvalidInput));
        }

        BACKEND_REQUEST.increment();

        let ttl = if let Some(ttl) = request.ttl() {
            NonZeroU64::new(ttl as u64)
        } else {
            None
        };

        match timeout(
            Duration::from_millis(200),
            client.set(&cache_name, key, &value, ttl),
        )
        .await
        {
            Ok(Ok(result)) => {
                match result.result {
                    MomentoSetStatus::OK => {
                        SET_STORED.increment();
                        if request.noreply() {
                            klog_set(
                                key,
                                request.flags(),
                                request.ttl().unwrap_or(0),
                                value.len(),
                                5,
                                0,
                            );
                        } else {
                            klog_set(
                                key,
                                request.flags(),
                                request.ttl().unwrap_or(0),
                                value.len(),
                                5,
                                8,
                            );
                            SESSION_SEND.increment();
                            SESSION_SEND_BYTE.add(8);
                            TCP_SEND_BYTE.add(8);
                            if let Err(e) = socket.write_all(b"STORED\r\n").await {
                                SESSION_SEND_EX.increment();
                                // hangup if we can't send a response back
                                return Err(e);
                            }
                        }
                    }
                    MomentoSetStatus::ERROR => {
                        SET_NOT_STORED.increment();
                        if request.noreply() {
                            klog_set(
                                key,
                                request.flags(),
                                request.ttl().unwrap_or(0),
                                value.len(),
                                9,
                                0,
                            );
                        } else {
                            klog_set(
                                key,
                                request.flags(),
                                request.ttl().unwrap_or(0),
                                value.len(),
                                9,
                                12,
                            );
                            SESSION_SEND.increment();
                            SESSION_SEND_BYTE.add(12);
                            TCP_SEND_BYTE.add(12);
                            // let client know this wasn't stored
                            if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                                SESSION_SEND_EX.increment();
                                // hangup if we can't send a response back
                                return Err(e);
                            }
                        }
                    }
                }
            }
            Ok(Err(MomentoError::LimitExceeded(_))) => {
                BACKEND_EX.increment();
                BACKEND_EX_RATE_LIMITED.increment();

                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
            Ok(Err(e)) => {
                error!("error for set: {}", e);

                BACKEND_EX.increment();
                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
            Err(_) => {
                // timeout
                BACKEND_EX.increment();
                BACKEND_EX_TIMEOUT.increment();
                SET_EX.increment();
                SET_NOT_STORED.increment();
                SESSION_SEND.increment();
                SESSION_SEND_BYTE.add(12);
                TCP_SEND_BYTE.add(12);

                // let client know this wasn't stored
                if let Err(e) = socket.write_all(b"NOT_STORED\r\n").await {
                    SESSION_SEND_EX.increment();
                    // hangup if we can't send a response back
                    return Err(e);
                }
            }
        }
    } else {
        SET_EX.increment();
        // invalid key
        SESSION_SEND.increment();
        SESSION_SEND_BYTE.add(7);
        TCP_SEND_BYTE.add(7);
        if socket.write_all(b"ERROR\r\n").await.is_err() {
            SESSION_SEND_EX.increment();
        }
        return Err(Error::from(ErrorKind::InvalidInput));
    }
    Ok(())
}
