use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Numeric {
    value: u64,
    noreply: bool,
}

impl Numeric {
    pub fn new(value: u64, noreply: bool) -> Self {
        Self { value, noreply }
    }
}

impl Compose for Numeric {
    fn compose(&self, session: &mut session::Session) {
        if !self.noreply {
            let _ = session.write_all(&format!("{}\r\n", self.value).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            response(b"0\r\n"),
            Ok((&b""[..], Response::numeric(0, false),))
        );

        assert_eq!(
            response(b"42 \r\n"),
            Ok((&b""[..], Response::numeric(42, false),))
        );
    }
}
