// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod builder;
mod worker_builder;

pub use builder::ProcessBuilder;
pub use worker_builder::WorkerBuilder;

use common::signal::Signal;
use queues::QueuePairs;
use std::thread::JoinHandle;

/// A structure which represents a running Pelikan cache process.
///
/// Note: for long-running daemon, be sure to call `wait()` on this structure to
/// block the process until the threads terminate. For use within tests, be sure
/// to call `shutdown()` to terminate the threads and block until termination.
pub struct Process {
    threads: Vec<JoinHandle<()>>,
    /// used to send signals to and from the admin thread
    signal_queue: QueuePairs<Signal, Signal>,
}

impl Process {
    /// Attempts to gracefully shutdown the `Process` by sending a shutdown to
    /// each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(mut self) {
        // this sends a shutdown to the admin thread, which will broadcast the
        // signal to all sibling threads in the process
        if self.signal_queue.broadcast(Signal::Shutdown).is_err() {
            fatal!("error sending shutdown signal to thread");
        }

        // try to wake the admin thread to process the signal
        if self.signal_queue.wake_all().is_err() {
            error!("error waking threads for shutdown");
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        for thread in self.threads {
            let _ = thread.join();
        }
    }
}
