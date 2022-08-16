use std::fs::File;
use std::io::{ErrorKind, Result, Write};
use std::os::unix::io::FromRawFd;

pub struct Waker {
    inner: File,
}

// a simple eventfd waker. based off the implementation in mio
impl Waker {
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