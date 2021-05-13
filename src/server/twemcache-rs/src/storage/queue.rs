
use rtrb::*;
use crate::*;


/// `RequestMessage`s are used to send a request from the workker thread to the
/// storage thread.
pub struct RequestMessage<Request> {
    pub request: Request,
    pub token: Token,
}

/// `RequestMessage`s are used to send responsed from the storage thread to the
/// worker thread.
pub struct ResponseMessage<Response> {
    pub response: Response,
    pub token: Token,
}


/// A `StorageQueue` is used to wrap the send and receive queues for the worker
/// threads.
pub struct StorageQueue<Request, Response> {
    pub(super) sender: Producer<RequestMessage<Request>>,
    pub(super) receiver: Consumer<ResponseMessage<Response>>,
    pub(super) waker: Arc<Waker>,
}

impl<Request, Response> StorageQueue<Request, Response> {
    // Try to receive a message back from the storage queue, returned messages
    // will contain the session write buffer with the response appended.
    pub fn try_recv(&mut self) -> Result<ResponseMessage<Response>, PopError> {
        self.receiver.pop()
    }

    // Try to send a message to the storage queue. Messages should contain the
    // parsed request and the session write buffer.
    pub fn try_send(&mut self, msg: RequestMessage<Request>) -> Result<(), PushError<RequestMessage<Request>>> {
        self.sender.push(msg)
    }

    // Notify the storage thread that it should wake and handle messages.
    pub fn wake(&self) -> Result<(), std::io::Error> {
        self.waker.wake()
    }
}