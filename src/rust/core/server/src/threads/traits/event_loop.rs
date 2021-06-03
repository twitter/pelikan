// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A trait defining common functions for event-based threads which operate on
//! sessions.

use std::io::{BufRead, ErrorKind, Write};

use mio::Token;

use crate::poll::Poll;

/// An `EventLoop` describes the functions which must be implemented for a basic
/// event loop and provides some default implementations and helper functions.
pub trait EventLoop {
    // the following functions must be implemented

    /// Provides access to the `Poll` structure which allows polling for new
    /// readiness events and managing registration for event sources.
    fn poll(&mut self) -> &mut Poll;

    /// Handle new data received for the `Session` with the provided `Token`.
    /// This will include parsing the incoming data and composing a response.
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error>;

    /// Handle a read event for the `Session` with the `Token`.
    fn do_read(&mut self, token: Token) -> Result<(), ()> {
        trace!("handling read for session: {}", token.0);

        if let Ok(session) = self.poll().get_mut_session(token) {
            // read from session to buffer
            match session.fill_buf().map(|b| b.len()) {
                Ok(0) => {
                    self.handle_hup(token);
                    Err(())
                }
                Ok(bytes) => {
                    trace!("read: {} bytes for session: {}", bytes, token.0);
                    if self.handle_data(token).is_err() {
                        self.handle_error(token);
                        Err(())
                    } else {
                        Ok(())
                    }
                }
                Err(e) => {
                    match e.kind() {
                        ErrorKind::WouldBlock => {
                            // spurious read
                            trace!("spurious read");
                            self.poll().reregister(token);
                            Ok(())
                        }
                        ErrorKind::Interrupted => self.do_read(token),
                        _ => {
                            // some read error
                            self.handle_error(token);
                            Err(())
                        }
                    }
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
        if let Ok(session) = self.poll().get_mut_session(token) {
            match session.flush() {
                Ok(_) => {
                    // if we wrote data but still have data in the read buffer
                    // attempt to process that data
                    if session.read_pending() != 0 {
                        if self.handle_data(token).is_err() {
                            self.handle_error(token);
                        } else {
                            self.poll().reregister(token);
                        }
                    } else {
                        self.poll().reregister(token);
                    }
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => {}
                    ErrorKind::Interrupted => self.do_write(token),
                    _ => {
                        self.handle_error(token);
                    }
                },
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
        let _ = self.poll().close_session(token);
    }

    /// Handle HUP (zero-length reads) for the `Session` with the `Token` by
    /// logging a message and closing the session.
    fn handle_hup(&mut self, token: Token) {
        trace!("handling hup for session: {}", token.0);
        debug!("Session closed by client");
        let _ = self.poll().close_session(token);
    }
}
