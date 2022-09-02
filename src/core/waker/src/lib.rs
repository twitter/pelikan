// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Provides a `Waker` trait to allow using the `Waker` from `mio` or a provided
//! `Waker` that uses eventfd directly (supported only on linux) interchangably.
//!
//! This is particularly useful in cases where some struct (such as a queue) may
//! be used with either `mio`-based event loops, or with io_uring. The `Waker`
//! provided by `mio` is not directly usable in io_uring based code due to the
//! fact that it must be registered to an event loop (such as epoll).

use core::sync::atomic::{AtomicU64, Ordering};

pub struct Waker {
    inner: Box<dyn GenericWaker>,
    pending: AtomicU64,
}

impl From<MioWaker> for Waker {
    fn from(other: MioWaker) -> Self {
        Self {
            inner: Box::new(other),
            pending: AtomicU64::new(0),
        }
    }
}

impl Waker {
    pub fn wake(&self) -> std::io::Result<()> {
        if self.pending.fetch_add(1, Ordering::Relaxed) == 0 {
            self.inner.wake()
        } else {
            Ok(())
        }
    }

    pub fn as_raw_fd(&self) -> Option<RawFd> {
        self.inner.as_raw_fd()
    }

    pub fn reset(&self) {
        self.pending.store(0, Ordering::Relaxed);
    }
}

pub trait GenericWaker: Send + Sync {
    fn wake(&self) -> std::io::Result<()>;

    fn as_raw_fd(&self) -> Option<RawFd>;
}

use std::os::unix::prelude::RawFd;

pub use mio::Waker as MioWaker;

impl GenericWaker for MioWaker {
    fn wake(&self) -> std::io::Result<()> {
        self.wake()
    }

    fn as_raw_fd(&self) -> Option<RawFd> {
        None
    }
}

#[cfg(target_os = "linux")]
pub use self::eventfd::EventfdWaker;

#[cfg(target_os = "linux")]
mod eventfd {
    use crate::*;
    use std::fs::File;
    use std::io::{ErrorKind, Result, Write};
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::os::unix::prelude::RawFd;

    pub struct EventfdWaker {
        inner: File,
    }

    // a simple eventfd waker. based off the implementation in mio
    impl EventfdWaker {
        pub fn new() -> Result<Self> {
            let ret = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
            if ret < 0 {
                Err(std::io::Error::new(
                    ErrorKind::Other,
                    "failed to create eventfd",
                ))
            } else {
                Ok(Self {
                    inner: unsafe { File::from_raw_fd(ret) },
                })
            }
        }

        pub fn wake(&self) -> Result<()> {
            match (&self.inner).write(&[1, 0, 0, 0, 0, 0, 0, 0]) {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // writing blocks if the counter would overflow, reset it
                        // and wake again
                        self.reset()?;
                        self.wake()
                    } else {
                        Err(e)
                    }
                }
            }
        }

        fn reset(&self) -> Result<()> {
            match (&self.inner).write(&[0, 0, 0, 0, 0, 0, 0, 0]) {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // we can ignore wouldblock during reset
                        Ok(())
                    } else {
                        Err(e)
                    }
                }
            }
        }
    }

    impl GenericWaker for EventfdWaker {
        fn wake(&self) -> Result<()> {
            self.wake()
        }

        fn as_raw_fd(&self) -> Option<RawFd> {
            Some(self.inner.as_raw_fd())
        }
    }

    impl From<EventfdWaker> for Waker {
        fn from(other: EventfdWaker) -> Self {
            Self {
                inner: Box::new(other),
                pending: AtomicU64::new(0),
            }
        }
    }
}
