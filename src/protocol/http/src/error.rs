// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::Response;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unable to parse request")]
    Unparseable(#[from] httparse::Error),
    #[error("Content-Length header was invalid")]
    BadContentLength,
    #[error("Content-Length header was missing")]
    MissingContentLength,
    #[error("method was unsupported")]
    BadRequestMethod,

    /// Contains the number of additional bytes needed to parse the rest of the
    /// request, if known.
    #[error("not enough data present to parse the whole request")]
    PartialRequest(Option<usize>),

    #[error("an internal error occurred: {0}")]
    InternalError(&'static str),
}

impl Error {
    pub fn to_response(&self) -> Response {
        match self {
            Self::Unparseable(e) => Response::builder(400)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(format!("Unable to parse request: {}", e).as_bytes()),
            Self::BadRequestMethod => Response::builder(405)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(
                    format!("Unsupported method, only GET, PUT, and DELETE are supported")
                        .as_bytes(),
                ),
            Self::BadContentLength => Response::builder(400)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(format!("Content-Length header was invalid").as_bytes()),
            Self::MissingContentLength => Response::builder(411)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(
                    format!("A Content-Length header is required for all PUT requests").as_bytes(),
                ),
            Self::InternalError(message) => Response::builder(500)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(message.as_bytes()),

            Self::PartialRequest(_) => Response::builder(500)
                .should_close(true)
                .header("Content-Type", b"text/plain")
                .body(b"internal server error"),
        }
    }
}
