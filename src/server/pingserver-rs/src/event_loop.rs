// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::metrics::Metrics;
use crate::session::Session;
use crate::{Stat, Token};

use mio::Poll;

use std::sync::Arc;

/// An `EventLoop` describes the functions which must be implemented for a basic
/// event loop and provides some default implementations and helper functions.
pub trait EventLoop {
    // the following functions must be implemented

    /// Provides access to an atomic-reference counted `Metrics` structure. This
    /// is used to allow consistent access to the `Metrics` structure and allow
    /// the default implementation of the related helper functions.
    fn metrics(&self) -> &Arc<Metrics<Stat>>;

    /// Provides access to the `Poll` structure which allows polling for new
    /// readiness events and managing registration for event sources.
    fn poll(&self) -> &Poll;

    /// Mutably borrow a `Session` from the event loop if a `Session` with that
    /// `Token` exists.
    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session>;

    /// Takes the `Session` out of the event loop if a `Session` with that
    /// `Token` exists.
    fn take_session(&mut self, token: Token) -> Option<Session>;

    /// Re-register the session with the provided `Token`.
    fn reregister(&mut self, token: Token);

    /// Handle new data received for the `Session` with the provided `Token`.
    /// This will include parsing the incoming data and composing a response.
    fn handle_data(&mut self, token: Token);

    // stats helper functions

    /// Increment the statistic counter.
    fn increment_count(&self, stat: Stat) {
        self.increment_count_n(stat, 1)
    }

    /// Increment the statistic counter by a provided count.
    fn increment_count_n(&self, stat: Stat, count: u64) {
        let _ = self.metrics().increment_counter(stat, count);
    }

    /// Handle a read event for the `Session` with the `Token`.
    fn do_read(&mut self, token: Token) -> Result<(), ()> {
        trace!("handling read for session: {}", token.0);

        if let Some(session) = self.get_mut_session(token) {
            // read from session to buffer
            match session.read() {
                Ok(Some(0)) => {
                    self.handle_hup(token);
                    Err(())
                }
                Ok(Some(bytes)) => {
                    trace!("read: {} bytes for session: {}", bytes, token.0);
                    self.handle_data(token);
                    Ok(())
                }
                Ok(None) => {
                    // spurious read
                    trace!("spurious read");
                    self.reregister(token);
                    Ok(())
                }
                Err(_) => {
                    // some read error
                    self.handle_error(token);
                    Err(())
                }
            }
        } else {
            trace!("attempted to read non-existent session: {}", token.0);
            Err(())
        }
    }

    /// Handle a write event for a `Session` with the `Token`.
    fn do_write(&mut self, token: Token) {
        trace!("handling write for session: {}", token.0);
        if let Some(session) = self.get_mut_session(token) {
            match session.flush() {
                Ok(Some(_)) => {
                    // if we wrote data but still have data in the read buffer
                    // attempt to process that data
                    if session.buffer().read_pending() != 0 {
                        self.handle_data(token);
                    }
                    // interests may have changed, so reregister
                    self.reregister(token);
                }
                Ok(None) => {
                    // spurious write
                }
                Err(_) => {
                    // some error writing
                    self.handle_error(token);
                }
            }
        } else {
            trace!("attempted to flush non-existent session: {}", token.0)
        }
    }

    /// Handle errors for the `Session` with the `Token` by logging a message
    /// and closing the session.
    fn handle_error(&mut self, token: Token) {
        trace!("handling error for session: {}", token.0);
        debug!("Error handling event");
        self.close(token);
    }

    /// Handle HUP (zero-length reads) for the `Session` with the `Token` by
    /// logging a message and closing the session.
    fn handle_hup(&mut self, token: Token) {
        trace!("handling hup for session: {}", token.0);
        debug!("Session closed by client");
        self.close(token);
    }

    /// Close the `Session` with the `Token`.
    fn close(&mut self, token: Token) {
        trace!("closing session: {}", token.0);
        if let Some(mut session) = self.take_session(token) {
            if session.deregister(self.poll()).is_err() {
                error!("Error deregistering");
            }
            session.close();
        } else {
            trace!("attempted to close non-existent session: {}", token.0);
        }
    }
}
