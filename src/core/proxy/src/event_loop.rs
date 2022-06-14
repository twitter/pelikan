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
        if let Ok(session) = self.poll().get_mut_session(token) {
            // read from session to buffer
            match session.session.fill_buf().map(|b| b.len()) {
                Ok(0) => {
                    trace!("hangup for session: {:?}", session.session);
                    let _ = self.poll().close_session(token);
                    Err(())
                }
                Ok(bytes) => {
                    trace!("read {} bytes for session: {:?}", bytes, session.session);
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
                            trace!("would block");
                            // spurious read
                            self.poll().reregister(token);
                            Ok(())
                        }
                        ErrorKind::Interrupted => {
                            trace!("interrupted");
                            self.do_read(token)
                        }
                        _ => {
                            trace!("error reading for session: {:?} {:?}", session.session, e);
                            // some read error
                            self.handle_error(token);
                            Err(())
                        }
                    }
                }
            }
        } else {
            warn!("attempted to read from non-existent session: {}", token.0);
            Err(())
        }
    }

    /// Handle a write event for a `Session` with the `Token`.
    fn do_write(&mut self, token: Token) {
        if let Ok(session) = self.poll().get_mut_session(token) {
            trace!("write for session: {:?}", session.session);
            match session.session.flush() {
                Ok(_) => {
                    self.poll().reregister(token);
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
            trace!("attempted to write to non-existent session: {}", token.0)
        }
    }

    /// Handle errors for the `Session` with the `Token` by logging a message
    /// and closing the session.
    fn handle_error(&mut self, token: Token) {
        if let Ok(session) = self.poll().get_mut_session(token) {
            trace!("handling error for session: {:?}", session.session);
            let _ = session.session.flush();
            let _ = self.poll().close_session(token);
        } else {
            trace!(
                "attempted to handle error for non-existent session: {}",
                token.0
            )
        }
    }
}
