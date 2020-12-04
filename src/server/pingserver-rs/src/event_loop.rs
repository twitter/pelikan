// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::session::Session;
use crate::{Stat, Token};

use mio::Poll;
use rustcommon_metrics::{AtomicU64, Metrics};

use std::sync::Arc;

pub trait EventLoop {
    fn metrics(&self) -> &Arc<Metrics<AtomicU64, AtomicU64>>;
    fn poll(&self) -> &Poll;
    fn get_mut_session<'a>(&'a mut self, token: Token) -> Option<&'a mut Session>;
    fn take_session(&mut self, token: Token) -> Option<Session>;
    fn reregister(&mut self, token: Token);
    fn handle_data(&mut self, token: Token);

    // stats helper functions
    fn increment_count(&self, stat: &Stat) {
        self.increment_count_n(stat, 1)
    }
    fn increment_count_n(&self, stat: &Stat, count: u64) {
        let _ = self.metrics().increment_counter(stat, count);
    }

    /// Handle a read event for the session given its token
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

    /// Handle a write event for a session given its token
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

    /// Handle errors
    fn handle_error(&mut self, token: Token) {
        trace!("handling error for session: {}", token.0);
        debug!("Error handling event");
        self.close(token);
    }

    /// Handle HUP and zero-length reads
    fn handle_hup(&mut self, token: Token) {
        trace!("handling hup for session: {}", token.0);
        debug!("Session closed by client");
        self.close(token);
    }

    /// Close a session given its token
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
