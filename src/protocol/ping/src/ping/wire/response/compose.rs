use crate::*;

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.

#[allow(unused_must_use)]
impl Compose for Response {
    fn compose(&self, dst: &mut dyn BufMut) {
        match self {
            Self::Pong => {
                dst.put_slice(b"PONG\r\n");
            }
        }
    }
}
