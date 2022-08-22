use crate::klog::klog_set;
use crate::*;

use protocol_resp::SetRequest;

pub async fn set(
    client: &mut SimpleCacheClient,
    cache_name: &str,
    socket: &mut tokio::net::TcpStream,
    request: &SetRequest,
) -> Result<(), Error> {
    SET.increment();

    if let Ok(key) = std::str::from_utf8(&request.key()) {
        let value = if let Ok(value) = std::str::from_utf8(&request.value()) {
            value.to_owned()
        } else {
            debug!("value is not valid utf8: {:?}", request.value());
            let _ = socket.write_all(b"-ERR invalid value\r\n").await;
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

        let ttl = match request.expire_time() {
            Some(protocol_resp::ExpireTime::Seconds(v)) => NonZeroU64::new(v as u64),
            Some(protocol_resp::ExpireTime::Milliseconds(v)) => {
                NonZeroU64::new(std::cmp::min(1, v / 1000 as u64))
            }
            Some(_) => {
                if socket.write_all(b"-ERR expire time\r\n").await.is_err() {
                    SESSION_SEND_EX.increment();
                }
                return Err(Error::from(ErrorKind::InvalidInput));
            }
            None => None,
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
                        klog_set(
                            key,
                            0,
                            ttl.map(|v| v.get()).unwrap_or(0) as u32,
                            value.len(),
                            5,
                            8,
                        );
                        SESSION_SEND.increment();
                        SESSION_SEND_BYTE.add(8);
                        TCP_SEND_BYTE.add(8);
                        if let Err(e) = socket.write_all(b"+OK\r\n").await {
                            SESSION_SEND_EX.increment();
                            // hangup if we can't send a response back
                            return Err(e);
                        }
                    }
                    MomentoSetStatus::ERROR => {
                        SET_NOT_STORED.increment();
                        klog_set(
                            key,
                            0,
                            ttl.map(|v| v.get()).unwrap_or(0) as u32,
                            value.len(),
                            9,
                            12,
                        );
                        SESSION_SEND.increment();
                        SESSION_SEND_BYTE.add(12);
                        TCP_SEND_BYTE.add(12);
                        // let client know this wasn't stored
                        if let Err(e) = socket.write_all(b"-ERR backend error\r\n").await {
                            SESSION_SEND_EX.increment();
                            // hangup if we can't send a response back
                            return Err(e);
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
                if let Err(e) = socket.write_all(b"-ERR ratelimit exceeded\r\n").await {
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
                if let Err(e) = socket.write_all(b"-ERR backend error\r\n").await {
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
                if let Err(e) = socket.write_all(b"-ERR backend error\r\n").await {
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
        if socket.write_all(b"-ERR invalid key\r\n").await.is_err() {
            SESSION_SEND_EX.increment();
        }
        return Err(Error::from(ErrorKind::InvalidInput));
    }
    Ok(())
}
