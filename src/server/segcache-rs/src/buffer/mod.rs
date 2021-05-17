//! Collection of buffer types, including parser implementations and unifying
//! traits.

mod bytesmut;

pub trait Buffer {
    fn extend(&mut self, data: &[u8]);
}
