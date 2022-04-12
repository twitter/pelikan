use crate::Compose;
use crate::Request;
use session::Session;
use std::io::Write;

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.

#[allow(unused_must_use)]
impl Compose for Request {
    fn compose(self, dst: &mut Session) {
        match self {
            Self::Ping => {
                dst.write_all(b"ping\r\n");
            }
        }
    }
}
