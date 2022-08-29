use crate::klog::klog_get;
use crate::*;

pub use protocol_memcache::{Request, RequestParser};

pub async fn get(
    client: &mut SimpleCacheClient,
    cache_name: &str,
    socket: &mut tokio::net::TcpStream,
    keys: &[Box<[u8]>],
) -> Result<(), Error> {
    GET.increment();

    // check if any of the keys are invalid before
    // sending the requests to the backend
    for key in keys.iter() {
        if std::str::from_utf8(key).is_err() {
            GET_EX.increment();

            // invalid key
            let _ = socket.write_all(b"ERROR\r\n").await;
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }

    let mut response_buf = Vec::new();

    for key in keys.iter() {
        BACKEND_REQUEST.increment();
        GET_KEY.increment();

        // we've already checked the keys, so we
        // know this unwrap is safe
        let key = std::str::from_utf8(key).unwrap();

        match timeout(Duration::from_millis(200), client.get(cache_name, key)).await {
            Ok(Ok(response)) => {
                match response.result {
                    MomentoGetStatus::ERROR => {
                        // we got some error from
                        // the backend.
                        BACKEND_EX.increment();

                        GET_KEY_EX.increment();

                        // TODO: what is the right
                        // way to handle this?
                        //
                        // currently ignoring and
                        // moving on to the next key
                    }
                    MomentoGetStatus::HIT => {
                        GET_KEY_HIT.increment();

                        let length = response.value.len();

                        let item_header = format!("VALUE {} 0 {}\r\n", key, length);

                        let response_len = 2 + item_header.len() + response.value.len();

                        klog_get(key, response_len);

                        response_buf.extend_from_slice(item_header.as_bytes());
                        response_buf.extend_from_slice(&response.value);
                        response_buf.extend_from_slice(b"\r\n");
                    }
                    MomentoGetStatus::MISS => {
                        GET_KEY_MISS.increment();

                        // we don't write anything for a miss

                        klog_get(key, 0);
                    }
                }
            }
            Ok(Err(MomentoError::LimitExceeded(_))) => {
                BACKEND_EX.increment();
                BACKEND_EX_RATE_LIMITED.increment();
                GET_KEY_EX.increment();
            }
            Ok(Err(e)) => {
                // we got some error from the momento client
                // log and incr stats and move on treating it
                // as a miss
                error!("error for get: {}", e);
                GET_KEY_EX.increment();
                BACKEND_EX.increment();
            }
            Err(_) => {
                // we had a timeout, incr stats and move on
                // treating it as a miss
                GET_KEY_EX.increment();
                BACKEND_EX.increment();
                BACKEND_EX_TIMEOUT.increment();
            }
        }
    }
    response_buf.extend_from_slice(b"END\r\n");

    SESSION_SEND.increment();
    SESSION_SEND_BYTE.add(response_buf.len() as _);
    TCP_SEND_BYTE.add(response_buf.len() as _);
    if let Err(e) = socket.write_all(&response_buf).await {
        SESSION_SEND_EX.increment();
        return Err(e);
    }
    Ok(())
}
