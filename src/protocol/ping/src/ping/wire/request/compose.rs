use crate::*;

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.

#[allow(unused_must_use)]
impl Compose for Request {
    fn compose(&self, dst: &mut dyn BufMut) -> usize {
        match self {
            Self::Ping => {
                dst.put_slice(b"ping\r\n");
                6
            }
        }
    }
}
