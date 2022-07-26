pub(crate) fn klog_get(key: &str, response_len: usize) {
    if response_len == 0 {
        klog!("\"get {}\" 0 {}", key, response_len);
    } else {
        klog!("\"get {}\" 4 {}", key, response_len);
    }
}

pub fn klog_set(
    key: &str,
    flags: u32,
    ttl: u32,
    value_len: usize,
    result_code: usize,
    response_len: usize,
) {
    klog!(
        "\"set {} {} {} {}\" {} {}",
        key,
        flags,
        ttl,
        value_len,
        result_code,
        response_len
    );
}