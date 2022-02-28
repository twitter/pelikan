use crate::response::error::MomentoError;

pub fn is_ttl_valid(ttl: &u32) -> Result<(), MomentoError> {
    // max_ttl will be 4294967 since 2^32 / 1000 = 4294967.296
    let max_ttl = u32::MAX / 1000 as u32;
    if *ttl > max_ttl {
        return Err(MomentoError::InvalidArgument(format!(
            "TTL provided, {}, needs to be less than the maximum TTL {}",
            ttl, max_ttl
        )));
    }
    return Ok(());
}
