use crate::util::*;
use crate::*;

use std::borrow::Cow;

fn escaped(bytes: &[u8]) -> Cow<str> {
    if bytes.iter().copied().all(|c| c.is_ascii()) {
        Cow::Borrowed(std::str::from_utf8(bytes).unwrap())
    } else {
        let string = bytes
            .iter()
            .copied()
            .flat_map(std::ascii::escape_default)
            .map(|c| c as char)
            .collect();

        Cow::Owned(string)
    }
}

#[test]
fn test_write_uri() {
    let mut out = vec![];
    let uri = Uri::new(b"/test uri");
    write_uri(&mut out, uri).unwrap();
    assert_eq!(escaped(&out), escaped(b"/test%20uri"));

    out.clear();
    let uri = unsafe { Uri::escaped_unchecked(b"/test uri") };
    write_uri(&mut out, uri).unwrap();
    assert_eq!(escaped(&out), escaped(b"/test uri"));

    out.clear();
    let uri = Uri::new(b"");
    assert_eq!(write_uri(&mut out, uri), Err(Error::InvalidUri));

    let mut out = [0u8; 5];
    let uri = Uri::new(b"/6char");
    assert_eq!(write_uri(&mut &mut out[..], uri), Err(Error::OutOfBuffer));
}

#[test]
fn test_write_version() {
    let mut out = vec![];
    write_version(&mut out, Version::Http10).unwrap();
    assert_eq!(escaped(&out), escaped(b"HTTP/1.0"));

    let mut out = vec![];
    write_version(&mut out, Version::Http11).unwrap();
    assert_eq!(escaped(&out), escaped(b"HTTP/1.1"));

    let mut out = vec![];
    write_version(&mut out, Version::Custom("MY-PROTOCOL/1.0")).unwrap();
    assert_eq!(escaped(&out), escaped(b"MY-PROTOCOL/1.0"));

    let mut out = vec![];
    assert_eq!(
        write_version(&mut out, Version::Custom("BAD VERSION")),
        Err(Error::InvalidVersion)
    );

    let mut bytes = [0u8; 5];
    assert_eq!(
        write_version(&mut &mut bytes[..], Version::Custom("6CHARS")),
        Err(Error::OutOfBuffer)
    );
}

#[test]
fn test_write_method() {
    let mut out = vec![];
    write_method(&mut out, Method::Get).unwrap();
    assert_eq!(escaped(&out), escaped(b"GET"));

    out.clear();
    write_method(&mut out, Method::Post).unwrap();
    assert_eq!(escaped(&out), escaped(b"POST"));

    out.clear();
    write_method(&mut out, Method::Delete).unwrap();
    assert_eq!(escaped(&out), escaped(b"DELETE"));

    out.clear();
    assert_eq!(
        write_method(&mut out, Method::Custom("CUSTOM METHOD")),
        Err(Error::InvalidMethod)
    );

    out.clear();
    let test_case = "LOOOOOOOOOOOOOOOOOOONG";
    write_method(&mut out, Method::Custom(test_case)).unwrap();
    assert_eq!(escaped(&out), test_case);

    let mut out = [0; 5];
    assert_eq!(
        write_method(&mut &mut out[..], Method::Delete),
        Err(Error::OutOfBuffer)
    );
}

#[test]
fn test_write_status() {
    let mut out = Vec::new();
    write_status(&mut out, Status::OK).unwrap();
    assert_eq!(escaped(&out), "200");

    out.clear();
    write_status(&mut out, Status::IM_A_TEAPOT).unwrap();
    assert_eq!(escaped(&out), "418");
}

#[test]
fn test_write_request_line() {
    let mut out = Vec::new();

    let method = Method::Post;
    let version = Version::Http11;
    let uri = Uri::new(b"/foo/bar/test/uri");

    write_request_line(&mut out, method, uri, version).unwrap();
    assert_eq!(escaped(&out), "POST /foo/bar/test/uri HTTP/1.1\r\n");
}

#[test]
fn test_write_status_line() {
    let mut out = Vec::new();

    let version = Version::Http11;
    let status = Status::INTERNAL_SERVER_ERROR;

    write_status_line(&mut out, version, status, "Internal Server Error").unwrap();

    assert_eq!(escaped(&out), "HTTP/1.1 500 Internal Server Error\r\n");
}

#[test]
fn test_headers() {
    let mut out = Vec::new();

    let mut rsp = HttpBuilder::response(&mut out, Version::Http11, Status::OK).unwrap();

    rsp.header("Test", "Blargh").unwrap();
    rsp.header("Server", "Test-Server-Foo").unwrap();
    rsp.header("X", "").unwrap();

    assert_eq!(
        rsp.header("Invalid With Spaces", "Foo").unwrap_err(),
        Error::InvalidHeaderKey
    );
    assert_eq!(rsp.header("", "Test").unwrap_err(), Error::InvalidHeaderKey);
    assert_eq!(
        rsp.header("X", "Invalid With LWS\r\n\r\nTest").unwrap_err(),
        Error::InvalidHeaderValue
    );

    #[rustfmt::skip]
    let expected = "\
        HTTP/1.1 200 OK\r\n\
        Test: Blargh\r\n\
        Server: Test-Server-Foo\r\n\
        X: \r\n\
        \r\n\
    ";

    rsp.finish().unwrap();

    assert_eq!(escaped(&out), expected);
}

#[test]
fn req_short() -> Result<()> {
    #[rustfmt::skip]
    let req_short = "\
        GET / HTTP/1.0\r\n\
        Host: example.com\r\n\
        Cookie: session=60; user_id=1\r\n\r\n\
    ";

    let mut buf = Vec::new();
    let mut req = HttpBuilder::request(&mut buf, Method::Get, Version::Http10, Uri::new(b"/"))?;

    req.header("Host", "example.com")?;
    req.header("Cookie", "session=60; user_id=1")?;
    req.finish()?;

    assert_eq!(escaped(&buf), req_short);

    Ok(())
}

#[test]
fn req_long() -> Result<()> {
    #[rustfmt::skip]
    let req_short = "\
        GET /wp-content/uploads/2010/03/hello-kitty-darth-vader-pink.jpg HTTP/1.1\r\n\
        Host: www.kittyhell.com\r\n\
        User-Agent: Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9\r\n\
        Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
        Accept-Language: ja,en-us;q=0.7,en;q=0.3\r\n\
        Accept-Encoding: gzip,deflate\r\n\
        Accept-Charset: Shift_JIS,utf-8;q=0.7,*;q=0.7\r\n\
        Keep-Alive: 115\r\n\
        Connection: keep-alive\r\n\
        Cookie: wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256\r\n\r\n\
    ";

    let mut buf = Vec::new();
    let mut req = HttpBuilder::request(
        &mut buf,
        Method::Get,
        Version::Http11,
        Uri::new(b"/wp-content/uploads/2010/03/hello-kitty-darth-vader-pink.jpg"),
    )?;

    req.header("Host", "www.kittyhell.com")?;
    req.header("User-Agent", "Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9")?;
    req.header(
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    )?;
    req.header("Accept-Language", "ja,en-us;q=0.7,en;q=0.3")?;
    req.header("Accept-Encoding", "gzip,deflate")?;
    req.header("Accept-Charset", "Shift_JIS,utf-8;q=0.7,*;q=0.7")?;
    req.header("Keep-Alive", "115")?;
    req.header("Connection", "keep-alive")?;
    req.header("Cookie", "wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256")?;

    req.finish()?;

    assert_eq!(escaped(&buf), req_short);

    Ok(())
}

#[test]
fn positive_numerical_header() -> Result<()> {
    #[rustfmt::skip]
    let header = "\
        GET / HTTP/1.1\r\n\
        Content-Length: 10\r\n\
        \r\n\
    ";

    let mut buf = Vec::new();
    let mut req = HttpBuilder::request(&mut buf, Method::Get, Version::Http11, Uri::new(b"/"))?;

    req.header("Content-Length", 10u128)?;
    req.finish()?;

    assert_eq!(escaped(&buf), header);

    Ok(())
}

#[test]
fn test_body_doesnt_bail() -> Result<()> {
    let mut buf = Vec::new();
    let req = HttpBuilder::response(&mut buf, Version::Http11, Status::OK)?;

    let mut body: &[u8] = b"TEST_STRING";
    req.body(&mut body)?;

    Ok(())
}
