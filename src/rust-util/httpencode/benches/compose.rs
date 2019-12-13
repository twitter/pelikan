use criterion::{criterion_group, criterion_main, Bencher, Criterion};
use httpencode::{HttpBuilder, Method, Uri, Version};

fn build_req_long(b: &mut Bencher) {
    let mut buf = Vec::new();
    buf.reserve(1 << 14);

    b.iter(|| -> Result<_, _> {
        buf.clear();

        let mut req = HttpBuilder::request(
            &mut buf,
            Method::Get,
            Version::Http11,
            Uri::new(b"/wp-content/uploads/2010/03/hello-kitty-darth-vader-pink.jpg")
        )?;

        req.header("Host", "www.kittyhell.com")?;
        req.header("User-Agent", "Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9")?;
        req.header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")?;
        req.header("Accept-Language", "ja,en-us;q=0.7,en;q=0.3")?;
        req.header("Accept-Encoding", "gzip,deflate")?;
        req.header("Accept-Charset", "Shift_JIS,utf-8;q=0.7,*;q=0.7")?;
        req.header("Keep-Alive", "115")?;
        req.header("Connection", "keep-alive")?;
        req.header("Cookie", "wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256")?;

        req.finish().map(|_| ())
    });
}

fn build_req_long_unsafe(b: &mut Bencher) {
    let mut buf = Vec::new();
    buf.reserve(1 << 14);

    b.iter(|| -> Result<_, _> {
        unsafe {
            buf.clear();

            let mut req = HttpBuilder::request(
                &mut buf,
                Method::Get,
                Version::Http11,
                Uri::escaped_unchecked(b"/wp-content/uploads/2010/03/hello-kitty-darth-vader-pink.jpg")
            )?;

            req.header_unchecked("Host", "www.kittyhell.com")?;
            req.header_unchecked("User-Agent", "Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9")?;
            req.header_unchecked("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")?;
            req.header_unchecked("Accept-Language", "ja,en-us;q=0.7,en;q=0.3")?;
            req.header_unchecked("Accept-Encoding", "gzip,deflate")?;
            req.header_unchecked("Accept-Charset", "Shift_JIS,utf-8;q=0.7,*;q=0.7")?;
            req.header_unchecked("Keep-Alive", "115")?;
            req.header_unchecked("Connection", "keep-alive")?;
            req.header_unchecked("Cookie", "wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256")?;

            req.finish().map(|_| ())
        }
    });
}

fn build_req_short(b: &mut Bencher) {
    let mut buf = Vec::new();
    buf.reserve(1 << 14);

    b.iter(|| -> Result<_, _> {
        buf.clear();

        let mut req = HttpBuilder::request(&mut buf, Method::Get, Version::Http11, Uri::new(b"/"))?;

        req.header("Host", "example.com")?;
        req.header("Cookie", "session=60; user_id=1")?;

        req.finish().map(|_| ())
    });
}

fn build_req_short_unsafe(b: &mut Bencher) {
    let mut buf = Vec::new();
    buf.reserve(1 << 14);

    b.iter(|| -> Result<_, _> {
        unsafe {
            buf.clear();

            let mut req = HttpBuilder::request(
                &mut buf,
                Method::Get,
                Version::Http11,
                Uri::escaped_unchecked(b"/"),
            )?;

            req.header_unchecked("Host", "example.com")?;
            req.header_unchecked("Cookie", "session=60; user_id=1")?;

            req.finish().map(|_| ())
        }
    });
}

pub fn benches(c: &mut Criterion) {
    c.bench_function("build_req_long", build_req_long);
    c.bench_function("build_req_long_unsafe", build_req_long_unsafe);
    c.bench_function("build_req_short", build_req_short);
    c.bench_function("build_req_short_unsafe", build_req_short_unsafe);
}

criterion_group!(group, benches);
criterion_main!(group);
