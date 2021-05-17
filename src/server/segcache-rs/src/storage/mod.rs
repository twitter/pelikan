mod segcache;

pub use self::segcache::*;

/// A trait which defines conversion of a Pelikan expiry into a TTL
pub trait GetTtl {
    /// Convert an expiry to a TTL based on the desired interpretation of the
    /// expiry field.
    fn get_ttl(&self, expiry: u32) -> u32;
}
