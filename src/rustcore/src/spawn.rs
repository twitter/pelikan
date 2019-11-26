use std::cell::Cell;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread::{self, ThreadId};

use tokio::task::JoinHandle;

thread_local! {
    static THREAD_ID: Cell<Option<ThreadId>> = Cell::new(None)
}

fn current_thread() -> ThreadId {
    THREAD_ID.with(|id| match id.get() {
        Some(thread) => thread,
        None => {
            let thread = thread::current().id();
            id.set(Some(thread));
            thread
        }
    })
}

pub fn spawn_local<F: Future<Output = ()> + 'static>(fut: F) -> JoinHandle<F::Output> {
    let fut = ThreadPinnedFuture::new(fut);

    tokio::spawn(fut)
}

#[pin_project]
pub(crate) struct ThreadPinnedFuture<F> {
    #[pin]
    inner: F,
    thread: Option<ThreadId>,
}

unsafe impl<F> Send for ThreadPinnedFuture<F> {}

impl<F> ThreadPinnedFuture<F> {
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            thread: None,
        }
    }
}

impl<F: Future> Future for ThreadPinnedFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<F::Output> {
        let thread = current_thread();
        let proj = self.project();

        match mem::replace(proj.thread, Some(thread)) {
            Some(prev) if prev == thread => (),
            None => (),
            Some(_) => panic!("Moved a ThreadPinnedFuture to a different thread!"),
        }

        proj.inner.poll(ctx)
    }
}
