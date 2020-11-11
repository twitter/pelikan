// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use bytes::{Buf, BufMut};

use crate::util::{
    lookup_status_line, validate_header_name, write_request_line, write_status_line,
};
use crate::{Error, HeaderValue, Method, Result, Status, Uri, Version};

/// Builder for HTTP requests.
#[derive(Debug)]
pub struct HttpBuilder<B: BufMut> {
    buf: B,
}

impl<B: BufMut> HttpBuilder<B> {
    /// Create a new request with the provided header line.
    ///
    /// # Note
    /// If this method fails it may be partially-written into the buffer.
    /// It is necessary to reset the buffer back externally if that happens.
    #[inline]
    pub fn request(mut buf: B, method: Method, version: Version, uri: Uri) -> Result<Self> {
        write_request_line(&mut buf, method, uri, version)?;

        Ok(Self { buf })
    }

    /// Create a new response from the provided status line.
    #[inline]
    pub fn response(buf: B, version: Version, status: Status) -> Result<Self> {
        Self::response_with_reason(
            buf,
            version,
            status,
            lookup_status_line(status).unwrap_or(" "),
        )
    }

    /// Create a new response from the provided status line.
    #[inline]
    pub fn response_with_reason(
        mut buf: B,
        version: Version,
        status: Status,
        reason: &str,
    ) -> Result<Self> {
        write_status_line(&mut buf, version, status, reason)?;

        Ok(Self { buf })
    }

    /// Add a new header to the request. This method does not check
    /// whether the given header has already been specified and whether
    /// it is valid to do so.
    ///
    /// # Note
    /// This method is atomic - if it fails then nothing will be written
    /// to the buffer.
    #[inline]
    pub fn header(&mut self, key: impl AsRef<[u8]>, val: impl HeaderValue) -> Result<&mut Self> {
        let key = key.as_ref();

        if !validate_header_name(key) {
            return Err(Error::InvalidHeaderKey);
        }
        if !val.validate() {
            return Err(Error::InvalidHeaderValue);
        }

        unsafe { self.header_unchecked(key, val) }
    }

    /// Add a new header to the request without checking to ensure that it's
    /// valid.
    ///
    /// # Note
    /// This method is atomic - if it fails then nothing will be written
    /// to the buffer.
    #[inline]
    pub unsafe fn header_unchecked(
        &mut self,
        key: impl AsRef<[u8]>,
        val: impl HeaderValue,
    ) -> Result<&mut Self> {
        let key = key.as_ref();

        if key.is_empty() {
            return Err(Error::InvalidHeaderKey);
        }

        let est_required = key.len() + val.est_len().unwrap_or(0) + b": \r\n".len();
        if self.buf.remaining_mut() < est_required {
            return Err(Error::OutOfBuffer);
        }

        self.buf.put_slice(key);
        self.buf.put_slice(b": ");
        val.put(&mut self.buf)?;
        self.buf.put_slice(b"\r\n");

        Ok(self)
    }

    /// Complete the HTTP header and return the underlying buffer.
    #[inline]
    pub fn finish(mut self) -> Result<B> {
        if self.buf.remaining_mut() < b"\r\n".len() {
            return Err(Error::OutOfBuffer);
        }

        self.buf.put_slice(b"\r\n");

        Ok(self.buf)
    }

    /// Complete the HTTP header and return the underlying buffer.
    pub fn body<I: Buf>(mut self, buf: &mut I) -> Result<B> {
        let len = buf.remaining();

        if self.buf.remaining_mut() < len + b"\r\n".len() {
            return Err(Error::OutOfBuffer);
        }

        self.buf.put_slice(b"\r\n");

        while buf.has_remaining() {
            let bytes = buf.bytes();
            self.buf.put_slice(bytes);

            let len = bytes.len();
            buf.advance(len);
        }

        Ok(self.buf)
    }

    /// Get the underlying buffer for this request object.
    pub fn into_buf(self) -> B {
        self.buf
    }

    /// Create a request from the underlying buffer.
    ///
    /// # Safety
    /// This function is unsafe since you can use it to create a syntactically
    /// invalid request.
    pub unsafe fn from_buf(buf: B) -> Self {
        Self { buf }
    }

    /// Get the number of remaining bytes within the underlying buffer.
    pub fn remaining(&self) -> usize {
        self.buf.remaining_mut()
    }
}
