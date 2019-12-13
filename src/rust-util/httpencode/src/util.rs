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

use bytes::BufMut;

use crate::{Error, Method, Result, Status, Uri, UriData, Version};

pub(crate) const OPTIONS: &[u8] = b"OPTIONS";
pub(crate) const GET: &[u8] = b"GET";
pub(crate) const HEAD: &[u8] = b"HEAD";
pub(crate) const POST: &[u8] = b"POST";
pub(crate) const PATCH: &[u8] = b"PATCH";
pub(crate) const PUT: &[u8] = b"PUT";
pub(crate) const DELETE: &[u8] = b"DELETE";
pub(crate) const TRACE: &[u8] = b"TRACE";
pub(crate) const CONNECT: &[u8] = b"CONNECT";

pub(crate) const HTTP_1_0: &[u8] = b"HTTP/1.0";
pub(crate) const HTTP_1_1: &[u8] = b"HTTP/1.1";

pub(crate) fn try_write<B: BufMut>(buf: &mut B, bytes: &[u8]) -> Result<()> {
    if buf.remaining_mut() < bytes.len() {
        return Err(Error::OutOfBuffer);
    }

    buf.put_slice(bytes);
    Ok(())
}

// Write out a HTTP method as determined by RFC2616.
//
// > 5.1.1 Method
// >
// > The Method  token indicates the method to be performed on the
// > resource identified by the Request-URI. The method is case-sensitive.
// >
// >     Method         = "OPTIONS"                ; Section 9.2
// >                    | "GET"                    ; Section 9.3
// >                    | "HEAD"                   ; Section 9.4
// >                    | "POST"                   ; Section 9.5
// >                    | "PUT"                    ; Section 9.6
// >                    | "DELETE"                 ; Section 9.7
// >                    | "TRACE"                  ; Section 9.8
// >                    | "CONNECT"                ; Section 9.9
// >                    | extension-method
// >     extension-method = token
// >
// > The list of methods allowed by a resource can be specified in an
// > Allow header field (section 14.7). The return code of the response
// > always notifies the client whether a method is currently allowed on a
// > resource, since the set of allowed methods can change dynamically. An
// > origin server SHOULD return the status code 405 (Method Not Allowed)
// > if the method is known by the origin server but not allowed for the
// > requested resource, and 501 (Not Implemented) if the method is
// > unrecognized or not implemented by the origin server. The methods GET
// > and HEAD MUST be supported by all general-purpose servers. All other
// > methods are OPTIONAL; however, if the above methods are implemented,
// > they MUST be implemented with the same semantics as those specified
// > in section 9.
pub(crate) fn write_method<B: BufMut>(buf: &mut B, method: Method) -> Result<()> {
    match method {
        Method::Options => try_write(buf, OPTIONS),
        Method::Get => try_write(buf, GET),
        Method::Head => try_write(buf, HEAD),
        Method::Post => try_write(buf, POST),
        Method::Put => try_write(buf, PUT),
        Method::Patch => try_write(buf, PATCH),
        Method::Delete => try_write(buf, DELETE),
        Method::Trace => try_write(buf, TRACE),
        Method::Connect => try_write(buf, CONNECT),
        Method::Custom(method) => {
            if validate_method(method) {
                try_write(buf, method.as_bytes())
            } else {
                Err(Error::InvalidMethod)
            }
        }
    }
}

// Write out a request URI as determined by RFC2616.
//
// > 5.1.2 Request-URI
// >
// > The Request-URI is a Uniform Resource Identifier (section 3.2) and
// > identifies the resource upon which to apply the request.
// >
// >     Request-URI    = "*" | absoluteURI | abs_path | authority
//
// `absoluteURI`, `abs_path`, and `authority` are defined in RC2396.
//
// > 3\. URI Syntactic Components
// >
// > The URI syntax is dependent upon the scheme.  In general, absolute
// > URI are written as follows:
// >
// >     <scheme>:<scheme-specific-part>
// >
// > An absolute URI contains the name of the scheme being used (<scheme>)
// > followed by a colon (":") and then a string (the <scheme-specific-
// > part>) whose interpretation depends on the scheme.
// >
// > The URI syntax does not require that the scheme-specific-part have
// > any general structure or set of semantics which is common among all
// > URI.  However, a subset of URI do share a common syntax for
// > representing hierarchical relationships within the namespace.  This
// > "generic URI" syntax consists of a sequence of four main components:
// >
// >     <scheme>://<authority><path>?<query>
// >
// > each of which, except <scheme>, may be absent from a particular URI.
// > For example, some URI schemes do not allow an <authority> component,
// > and others do not use a <query> component.
//
// Where `unreserved` is defined as
//
// > 2.3. Unreserved Characters
// >
// > Data characters that are allowed in a URI but do not have a reserved
// > purpose are called unreserved.  These include upper and lower case
// > letters, decimal digits, and a limited set of punctuation marks and
// > symbols.
// >
// >     unreserved  = alphanum | mark
// >     mark        = "-" | "_" | "." | "!" | "~" | "*" | "'" | "(" | ")"
// >
// > Unreserved characters can be escaped without changing the semantics
// > of the URI, but this should not be done unless the URI is being used
// > in a context that does not allow the unescaped character to appear.
//
// and `escaped` is defined as
//
// > 2.4.1. Escaped Encoding
// >
// > An escaped octet is encoded as a character triplet, consisting of the
// > percent character "%" followed by the two hexadecimal digits
// > representing the octet code. For example, "%20" is the escaped
// > encoding for the US-ASCII space character.
// >
// >     escaped     = "%" hex hex
// >     hex         = digit | "A" | "B" | "C" | "D" | "E" | "F" |
// >                           "a" | "b" | "c" | "d" | "e" | "f"
pub(crate) fn write_uri<B: BufMut>(buf: &mut B, uri: Uri) -> Result<()> {
    pub(crate) fn is_valid(byte: u8) -> bool {
        // Bit-packed lookup table of valid unescaped characters.
        //
        // This corresponds to the characters as defined by
        // RFC2396 section 2.
        //
        // Note that no byte above 128 is valid in a URI.
        const LOOKUP: u128 = 0x47FFFFFE87FFFFFF2FFFFFD200000000;

        return (byte < 128) & (((LOOKUP.wrapping_shr(byte as u32)) & 1) != 0);
    }

    match uri.data {
        UriData::Unescaped(path) => {
            if path.is_empty() {
                return Err(Error::InvalidUri);
            }

            write_percent_escaped(buf, path, is_valid)
        }
        UriData::Escaped(path) => try_write(buf, path),
    }
}

// Write out a string and percent-escape any invalid characters within.
fn write_percent_escaped<B, F>(buf: &mut B, path: &[u8], is_valid: F) -> Result<()>
where
    B: BufMut,
    F: Fn(u8) -> bool,
{
    fn next_invalid<F: Fn(u8) -> bool>(bytes: &[u8], is_valid: &F) -> Option<(usize, u8)> {
        for i in 0..bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };

            if !is_valid(b) {
                return Some((i, b));
            }
        }

        None
    }

    fn hex_encode(byte: u8) -> u8 {
        let byte = byte & 0xF;
        match byte {
            0x0..=0x9 => b'0' + byte,
            0xA..=0xF => b'A' + byte - 0xA,
            _ => unreachable!(),
        }
    }

    fn percent_encode<B: BufMut>(buf: &mut B, byte: u8) -> Result<()> {
        let slice = [b'%', hex_encode(byte >> 4), hex_encode(byte)];

        try_write(buf, &slice)
    }

    let mut bytes = path;

    while !bytes.is_empty() {
        let advance = if let Some((idx, byte)) = next_invalid(bytes, &is_valid) {
            try_write(buf, &bytes[..idx])?;
            percent_encode(buf, byte)?;
            idx + 1
        } else {
            try_write(buf, &bytes)?;
            bytes.len()
        };

        bytes = &bytes[advance..];
    }

    Ok(())
}

pub(crate) fn write_version<B: BufMut>(buf: &mut B, version: Version) -> Result<()> {
    match version {
        Version::Http10 => try_write(buf, HTTP_1_0),
        Version::Http11 => try_write(buf, HTTP_1_1),
        Version::Custom(version) => {
            if validate_version(version) {
                try_write(buf, version.as_bytes())
            } else {
                Err(Error::InvalidVersion)
            }
        }
    }
}

// Write out a request line as determined by RCP2616.
//
// > 5.1 Request-Line
// >
// > The Request-Line begins with a method token, followed by the
// > Request-URI and the protocol version, and ending with CRLF. The
// > elements are separated by SP characters. No CR or LF is allowed
// > except in the final CRLF sequence.
// >
// >     Request-Line = Method SP Request-URI SP HTTP-Version CRLF
pub(crate) fn write_request_line<B: BufMut>(
    buf: &mut B,
    method: Method,
    uri: Uri,
    version: Version,
) -> Result<()> {
    write_method(buf, method)?;
    buf.put_u8(b' ');
    write_uri(buf, uri)?;
    buf.put_u8(b' ');
    write_version(buf, version)?;
    buf.put_slice(b"\r\n");

    Ok(())
}

// Write out a u16 in decimal to the buffer.
pub(crate) fn write_u16<B: BufMut>(buf: &mut B, mut num: u16) -> Result<()> {
    let mut bytes = [0u8; 5];
    let mut idx = 0;

    while num != 0 {
        bytes[idx] = (num % 10) as u8;
        num /= 10;
        idx += 1;
    }

    if idx > buf.remaining_mut() {
        return Err(Error::OutOfBuffer);
    }

    for i in (0..idx).rev() {
        buf.put_u8(b'0' + bytes[i]);
    }

    Ok(())
}

// Write out a status code + reason phrase as specified in RFC 7370.
//
// > The status-code element is a 3-digit integer code describing the
// > result of the server's attempt to understand and satisfy the client's
// > corresponding request.  The rest of the response message is to be
// > interpreted in light of the semantics defined for that status code.
// > See Section 6 of [RFC7231] for information about the semantics of
// > status codes, including the classes of status code (indicated by the
// > first digit), the status codes defined by this specification,
// > considerations for the definition of new status codes, and the IANA
// > registry.
// >
// >     status-code    = 3DIGIT
// >
// > The reason-phrase element exists for the sole purpose of providing a
// > textual description associated with the numeric status code, mostly
// > out of deference to earlier Internet application protocols that were
// > more frequently used with interactive text clients.  A client SHOULD
// > ignore the reason-phrase content.
// >
// >     reason-phrase  = *( HTAB / SP / VCHAR / obs-text )
pub(crate) fn write_status<B: BufMut>(buf: &mut B, status: Status) -> Result<()> {
    write_u16(buf, status.code)
}

// > 3.1.2.  Status Line
// >
// > The first line of a response message is the status-line, consisting
// > of the protocol version, a space (SP), the status code, another
// > space, a possibly empty textual phrase describing the status code,
// > and ending with CRLF.
// >
// >     status-line = HTTP-version SP status-code SP reason-phrase CRLF
// >
// > The status-code element is a 3-digit integer code describing the
// > result of the server's attempt to understand and satisfy the client's
// > corresponding request.  The rest of the response message is to be
// > interpreted in light of the semantics defined for that status code.
// > See Section 6 of [RFC7231] for information about the semantics of
// > status codes, including the classes of status code (indicated by the
// > first digit), the status codes defined by this specification,
// > considerations for the definition of new status codes, and the IANA
// > registry.
// >
// >     status-code    = 3DIGIT
// >
// > The reason-phrase element exists for the sole purpose of providing a
// > textual description associated with the numeric status code, mostly
// > out of deference to earlier Internet application protocols that were
// > more frequently used with interactive text clients.  A client SHOULD
// > ignore the reason-phrase content.
// >
// >     reason-phrase  = *( HTAB / SP / VCHAR / obs-text )
pub(crate) fn write_status_line<B: BufMut>(
    buf: &mut B,
    version: Version,
    status: Status,
    reason: &str,
) -> Result<()> {
    write_version(buf, version)?;
    try_write(buf, b" ")?;
    write_status(buf, status)?;
    try_write(buf, b" ")?;
    try_write(buf, reason.as_bytes())?;
    try_write(buf, b"\r\n")?;

    Ok(())
}

// We're already outside of the HTTP standard here so
// this currently just checks that the method doesn't
// contain a space or a newline.
pub(crate) fn validate_method(method: &str) -> bool {
    !method
        .as_bytes()
        .iter()
        .copied()
        .any(|c| c == b' ' || c == b'\r' || c == b'\n')
}

// We're already outside the HTTP standard here so just
// check to see that the version doesn't contain a space.
pub(crate) fn validate_version(version: &str) -> bool {
    !version.as_bytes().iter().copied().any(|c| c == b' ')
}

// Validate a header name as defined by RFC7230. This implementation
// does not allow for any obsolete syntax.
//
// > Each header field consists of a case-insensitive field name followed
// > by a colon (":"), optional leading whitespace, the field value, and
// > optional trailing whitespace.
// >
// >     header-field   = field-name ":" OWS field-value OWS
// >
// >     field-name     = token
// >     field-value    = *( field-content / obs-fold )
// >     field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
// >     field-vchar    = VCHAR / obs-text
// >
// >     obs-fold       = CRLF 1*( SP / HTAB )
// >                    ; obsolete line folding
// >                    ; see Section 3.2.4
// >
// > The field-name token labels the corresponding field-value as having
// > the semantics defined by that header field.  For example, the Date
// > header field is defined in Section 7.1.1.2 of [RFC7231] as containing
// > the origination timestamp for the message in which it appears.
//
// `token` is defined in RFC2616 Section 2.2
// >     token          = 1*<any CHAR except CTLs or separators>
// >     separators     = "(" | ")" | "<" | ">" | "@"
// >                    | "," | ";" | ":" | "\" | <">
// >                    | "/" | "[" | "]" | "?" | "="
// >                    | "{" | "}" | SP | HT
pub(crate) fn validate_header_name(header: &[u8]) -> bool {
    fn is_valid(byte: u8) -> bool {
        // Bit-packed lookup table of all characters that
        // are valid within `token`.
        const LOOKUP: u128 = 0x57FFFFFFC7FFFFFE03FF6CFE00000000;

        (byte < 128) & ((LOOKUP.wrapping_shr(byte as u32) & 1) != 0)
    }

    header.iter().copied().all(is_valid)
}

// Validate a header value as defined by RFC7230. This implementation
// does not allow for any obsolete syntax.
//
// > Each header field consists of a case-insensitive field name followed
// > by a colon (":"), optional leading whitespace, the field value, and
// > optional trailing whitespace.
// >
// >     header-field   = field-name ":" OWS field-value OWS
// >
// >     field-name     = token
// >     field-value    = *( field-content / obs-fold )
// >     field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
// >     field-vchar    = VCHAR / obs-text
// >
// >     obs-fold       = CRLF 1*( SP / HTAB )
// >                    ; obsolete line folding
// >                    ; see Section 3.2.4
// >
// > The field-name token labels the corresponding field-value as having
// > the semantics defined by that header field.  For example, the Date
// > header field is defined in Section 7.1.1.2 of [RFC7231] as containing
// > the origination timestamp for the message in which it appears.
pub(crate) fn validate_header_field(field: &[u8]) -> bool {
    field
        .iter()
        .copied()
        .all(|c| c.is_ascii_graphic() || c == b' ' || c == b'\t')
}

// Status lines sourced from [here][IANA].
//
// [IANA]: https://www.iana.org/assignments/http-status-codes/http-status-codes.xhtml
pub(crate) fn lookup_status_line(status: Status) -> Option<&'static str> {
    Some(match status.code {
        // 1xx: Informational
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",

        // 2xx: Success
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoratative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi Status",
        208 => "Already Reported",
        // 209 - 225 Unassigned
        226 => "IM Used",

        // 3xx: Redirection
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        306 => "Switch Proxy", // This status code is obsolete
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",

        // 4xx: Client Error
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Required",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Large",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a Teapot",
        // 419 - 420 Unassigned
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        // 427 Unassigned
        428 => "Precondition Required",
        429 => "Too Many Requests",
        // 432 - 450 Unassigned
        451 => "Unavailable for Legal Reasons",

        // 5xx: Server Error
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Time-out",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        // 509 Unassigned
        510 => "Not Extended",
        509 => "Network Authentication Required",

        _ => return None,
    })
}
